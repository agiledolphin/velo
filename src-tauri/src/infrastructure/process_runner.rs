use std::{ffi::OsString, future::Future, path::PathBuf, pin::Pin, process::Stdio, time::Duration};

use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    task::JoinHandle,
    time::timeout,
};

pub type ProcessFuture<'a> =
    Pin<Box<dyn Future<Output = Result<ProcessOutput, ProcessError>> + Send + 'a>>;

pub trait ProcessRunner: Send + Sync + 'static {
    fn run<'a>(&'a self, arguments: &'a [OsString]) -> ProcessFuture<'a>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessOutput {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessError {
    SpawnFailed,
    WaitFailed,
    ReadFailed,
    TimedOut,
    OutputTooLarge,
}

pub struct RestrictedProcessRunner {
    executable: PathBuf,
    timeout: Duration,
    max_output_bytes: usize,
}

impl RestrictedProcessRunner {
    pub fn new(executable: impl Into<PathBuf>, timeout: Duration, max_output_bytes: usize) -> Self {
        let executable = executable.into();
        assert!(
            executable.is_absolute(),
            "process executable path must be absolute"
        );
        assert!(timeout > Duration::ZERO, "process timeout must be positive");
        assert!(
            max_output_bytes > 0 && max_output_bytes < usize::MAX,
            "process output limit must be positive"
        );

        Self {
            executable,
            timeout,
            max_output_bytes,
        }
    }
}

impl ProcessRunner for RestrictedProcessRunner {
    fn run<'a>(&'a self, arguments: &'a [OsString]) -> ProcessFuture<'a> {
        Box::pin(async move {
            let mut command = Command::new(&self.executable);
            command
                .args(arguments)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);

            let mut child = command.spawn().map_err(|_| ProcessError::SpawnFailed)?;
            let stdout = child.stdout.take().ok_or(ProcessError::ReadFailed)?;
            let stderr = child.stderr.take().ok_or(ProcessError::ReadFailed)?;
            let stdout_task = tokio::spawn(read_bounded(stdout, self.max_output_bytes));
            let stderr_task = tokio::spawn(read_bounded(stderr, self.max_output_bytes));

            let status = match timeout(self.timeout, child.wait()).await {
                Ok(Ok(status)) => status,
                Ok(Err(_)) => {
                    stop_child(&mut child, &stdout_task, &stderr_task).await;
                    return Err(ProcessError::WaitFailed);
                }
                Err(_) => {
                    stop_child(&mut child, &stdout_task, &stderr_task).await;
                    return Err(ProcessError::TimedOut);
                }
            };

            let stdout = join_reader(stdout_task).await?;
            let stderr = join_reader(stderr_task).await?;

            Ok(ProcessOutput {
                success: status.success(),
                exit_code: status.code(),
                stdout,
                stderr,
            })
        })
    }
}

async fn read_bounded(
    reader: impl AsyncRead + Unpin,
    max_output_bytes: usize,
) -> Result<Vec<u8>, ProcessError> {
    let mut bytes = Vec::with_capacity(max_output_bytes.min(8 * 1024));
    reader
        .take((max_output_bytes + 1) as u64)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| ProcessError::ReadFailed)?;

    if bytes.len() > max_output_bytes {
        return Err(ProcessError::OutputTooLarge);
    }

    Ok(bytes)
}

async fn join_reader(
    task: JoinHandle<Result<Vec<u8>, ProcessError>>,
) -> Result<Vec<u8>, ProcessError> {
    task.await.map_err(|_| ProcessError::ReadFailed)?
}

async fn stop_child(
    child: &mut tokio::process::Child,
    stdout_task: &JoinHandle<Result<Vec<u8>, ProcessError>>,
    stderr_task: &JoinHandle<Result<Vec<u8>, ProcessError>>,
) {
    let _ = child.kill().await;
    let _ = child.wait().await;
    stdout_task.abort();
    stderr_task.abort();
}

#[cfg(test)]
mod tests {
    use std::{io::Write, time::Duration};

    use super::*;

    const ECHO_HELPER: &str = "infrastructure::process_runner::tests::helper_echo";
    const SLEEP_HELPER: &str = "infrastructure::process_runner::tests::helper_sleep";
    const LARGE_HELPER: &str = "infrastructure::process_runner::tests::helper_large_output";

    fn helper_arguments(test_name: &str) -> Vec<OsString> {
        ["--ignored", "--exact", test_name, "--nocapture"]
            .into_iter()
            .map(OsString::from)
            .collect()
    }

    #[tokio::test]
    async fn captures_output_from_the_fixed_executable() {
        let runner = RestrictedProcessRunner::new(
            std::env::current_exe().expect("test executable should be available"),
            Duration::from_secs(2),
            8 * 1024,
        );

        let output = runner
            .run(&helper_arguments(ECHO_HELPER))
            .await
            .expect("helper process should succeed");

        assert!(output.success);
        assert_eq!(output.exit_code, Some(0));
        assert!(String::from_utf8_lossy(&output.stdout).contains("velo-process-runner"));
    }

    #[tokio::test]
    async fn terminates_a_process_after_the_timeout() {
        let runner = RestrictedProcessRunner::new(
            std::env::current_exe().expect("test executable should be available"),
            Duration::from_millis(50),
            8 * 1024,
        );

        let error = runner
            .run(&helper_arguments(SLEEP_HELPER))
            .await
            .expect_err("slow helper should time out");

        assert_eq!(error, ProcessError::TimedOut);
    }

    #[tokio::test]
    async fn rejects_output_above_the_configured_limit() {
        let runner = RestrictedProcessRunner::new(
            std::env::current_exe().expect("test executable should be available"),
            Duration::from_secs(2),
            256,
        );

        let error = runner
            .run(&helper_arguments(LARGE_HELPER))
            .await
            .expect_err("large helper output should be rejected");

        assert_eq!(error, ProcessError::OutputTooLarge);
    }

    #[tokio::test]
    async fn passes_shell_metacharacters_as_literal_arguments() {
        let runner = RestrictedProcessRunner::new(
            std::env::current_exe().expect("test executable should be available"),
            Duration::from_secs(2),
            8 * 1024,
        );
        let arguments = vec![OsString::from("; printf velo-shell-injection")];

        let output = runner
            .run(&arguments)
            .await
            .expect("test process should remain bounded");

        assert!(
            !String::from_utf8_lossy(&output.stdout)
                .lines()
                .any(|line| line == "velo-shell-injection")
        );
    }

    #[test]
    #[should_panic(expected = "process executable path must be absolute")]
    fn rejects_relative_executable_paths() {
        let _ = RestrictedProcessRunner::new("binaries/yt-dlp", Duration::from_secs(2), 8 * 1024);
    }

    #[test]
    #[ignore]
    fn helper_echo() {
        println!("velo-process-runner");
    }

    #[test]
    #[ignore]
    fn helper_sleep() {
        std::thread::sleep(Duration::from_secs(1));
    }

    #[test]
    #[ignore]
    fn helper_large_output() {
        let bytes = vec![b'x'; 4 * 1024];
        std::io::stdout()
            .write_all(&bytes)
            .expect("helper should write test output");
    }
}
