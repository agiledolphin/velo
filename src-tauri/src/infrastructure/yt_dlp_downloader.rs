use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Stdio,
};

use serde::Deserialize;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, BufReader},
    process::Command,
    sync::{mpsc, watch},
    task::JoinHandle,
};

use crate::{
    application::{DownloadEngine, DownloadFuture, DownloadOutcome},
    domain::{DownloadFailure, DownloadProgress, DownloadTask},
};

const PROGRESS_PREFIX: &str = "VELO_PROGRESS:";
const MAX_STDOUT_BYTES: usize = 32 * 1024 * 1024;
const MAX_STDERR_BYTES: usize = 1024 * 1024;
const MAX_LINE_BYTES: usize = 64 * 1024;

pub struct YtDlpDownloader {
    executable: PathBuf,
}

impl YtDlpDownloader {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        let executable = executable.into();
        assert!(
            executable.is_absolute(),
            "download executable path must be absolute"
        );
        Self { executable }
    }
}

impl DownloadEngine for YtDlpDownloader {
    fn download<'a>(
        &'a self,
        task: &'a DownloadTask,
        cancellation: watch::Receiver<bool>,
        progress: mpsc::UnboundedSender<DownloadProgress>,
    ) -> DownloadFuture<'a> {
        Box::pin(async move { self.run(task, cancellation, progress).await })
    }
}

impl YtDlpDownloader {
    async fn run(
        &self,
        task: &DownloadTask,
        mut cancellation: watch::Receiver<bool>,
        progress: mpsc::UnboundedSender<DownloadProgress>,
    ) -> Result<DownloadOutcome, DownloadFailure> {
        let mut command = Command::new(&self.executable);
        command
            .args(download_arguments(task))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = command
            .spawn()
            .map_err(|_| DownloadFailure::engine_unavailable())?;
        let stdout = child.stdout.take().ok_or_else(DownloadFailure::failed)?;
        let stderr = child.stderr.take().ok_or_else(DownloadFailure::failed)?;
        let stdout_task = tokio::spawn(read_progress(stdout, progress));
        let stderr_task = tokio::spawn(read_bounded(stderr, MAX_STDERR_BYTES));

        let status = tokio::select! {
            biased;
            _ = cancellation.changed() => {
                stop_child(&mut child, &stdout_task, &stderr_task).await;
                return Ok(DownloadOutcome::Cancelled);
            }
            status = child.wait() => status.map_err(|_| DownloadFailure::failed())?,
        };

        let progress_result = stdout_task.await.map_err(|_| DownloadFailure::failed())?;
        let stderr_result = stderr_task.await.map_err(|_| DownloadFailure::failed())?;
        progress_result?;
        stderr_result?;

        if !status.success() || !Path::new(&task.destination_path).is_file() {
            return Err(DownloadFailure::failed());
        }

        Ok(DownloadOutcome::Completed)
    }
}

fn download_arguments(task: &DownloadTask) -> Vec<OsString> {
    [
        "--ignore-config",
        "--no-plugin-dirs",
        "--no-js-runtimes",
        "--no-remote-components",
        "--no-exec",
        "--no-cache-dir",
        "--no-update",
        "--no-playlist",
        "--no-warnings",
        "--no-overwrites",
        "--no-continue",
        "--part",
        "--no-mtime",
        "--newline",
        "--progress",
        "--progress-delta",
        "0.25",
        "--progress-template",
        "download:VELO_PROGRESS:%(progress)j",
        "--format",
        task.format_id.as_str(),
        "--output",
        task.destination_path.as_str(),
        "--",
        task.source_url.as_str(),
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}

async fn read_progress(
    reader: impl AsyncRead + Unpin,
    progress: mpsc::UnboundedSender<DownloadProgress>,
) -> Result<(), DownloadFailure> {
    let mut reader = BufReader::new(reader);
    let mut line = Vec::new();
    let mut total_bytes = 0usize;
    let mut exceeded_limit = false;

    loop {
        line.clear();
        let count = reader
            .read_until(b'\n', &mut line)
            .await
            .map_err(|_| DownloadFailure::failed())?;
        if count == 0 {
            break;
        }
        total_bytes = total_bytes.saturating_add(count);
        if line.len() > MAX_LINE_BYTES || total_bytes > MAX_STDOUT_BYTES {
            exceeded_limit = true;
            continue;
        }

        if let Some(parsed) = parse_progress_line(&String::from_utf8_lossy(&line)) {
            let _ = progress.send(parsed);
        }
    }

    if exceeded_limit {
        Err(DownloadFailure::output_too_large())
    } else {
        Ok(())
    }
}

async fn read_bounded(reader: impl AsyncRead + Unpin, limit: usize) -> Result<(), DownloadFailure> {
    let mut reader = reader;
    let mut buffer = [0u8; 8 * 1024];
    let mut total_bytes = 0usize;

    loop {
        let count = reader
            .read(&mut buffer)
            .await
            .map_err(|_| DownloadFailure::failed())?;
        if count == 0 {
            break;
        }
        total_bytes = total_bytes.saturating_add(count);
    }

    if total_bytes > limit {
        Err(DownloadFailure::output_too_large())
    } else {
        Ok(())
    }
}

async fn stop_child(
    child: &mut tokio::process::Child,
    stdout_task: &JoinHandle<Result<(), DownloadFailure>>,
    stderr_task: &JoinHandle<Result<(), DownloadFailure>>,
) {
    let _ = child.kill().await;
    let _ = child.wait().await;
    stdout_task.abort();
    stderr_task.abort();
}

#[derive(Deserialize)]
struct RawProgress {
    downloaded_bytes: Option<f64>,
    total_bytes: Option<f64>,
    total_bytes_estimate: Option<f64>,
    speed: Option<f64>,
    eta: Option<f64>,
}

fn parse_progress_line(line: &str) -> Option<DownloadProgress> {
    let payload = line.trim().strip_prefix(PROGRESS_PREFIX)?;
    let raw: RawProgress = serde_json::from_str(payload).ok()?;
    Some(DownloadProgress {
        downloaded_bytes: finite_u64(raw.downloaded_bytes).unwrap_or(0),
        total_bytes: finite_u64(raw.total_bytes.or(raw.total_bytes_estimate)),
        speed_bytes_per_second: finite_u64(raw.speed),
        eta_seconds: finite_u64(raw.eta),
    })
}

fn finite_u64(value: Option<f64>) -> Option<u64> {
    value
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| value.round() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::DownloadTaskId;

    fn task() -> DownloadTask {
        DownloadTask::new(
            DownloadTaskId::new("task-1").expect("task ID should be valid"),
            "https://video.example/watch?v=1&unsafe=true",
            "Title",
            "137/mp4",
            std::env::temp_dir().join("video.mp4").to_string_lossy(),
            "mp4",
        )
        .expect("task should be valid")
    }

    #[test]
    fn uses_a_fixed_download_argument_contract() {
        let arguments = download_arguments(&task());
        let strings = arguments
            .iter()
            .map(|value| value.to_string_lossy())
            .collect::<Vec<_>>();

        assert!(strings.iter().any(|value| value == "--ignore-config"));
        assert!(strings.iter().any(|value| value == "--no-overwrites"));
        assert!(strings.iter().any(|value| value == "--no-exec"));
        assert_eq!(strings[strings.len() - 2], "--");
        assert_eq!(
            strings.last().expect("URL"),
            "https://video.example/watch?v=1&unsafe=true"
        );
    }

    #[test]
    fn parses_machine_readable_progress() {
        let progress = parse_progress_line(
            r#"VELO_PROGRESS:{"downloaded_bytes":512,"total_bytes":1024,"total_bytes_estimate":null,"speed":256.4,"eta":2}"#,
        )
        .expect("progress should parse");

        assert_eq!(progress.downloaded_bytes, 512);
        assert_eq!(progress.total_bytes, Some(1024));
        assert_eq!(progress.speed_bytes_per_second, Some(256));
        assert_eq!(progress.eta_seconds, Some(2));
        assert!(parse_progress_line("WARNING: ignored").is_none());
    }

    #[tokio::test]
    #[ignore = "requires an explicit public video URL, format ID, verified yt-dlp, and network"]
    async fn downloads_configured_real_format_with_progress() {
        let source_url = std::env::var("VELO_INTEGRATION_DOWNLOAD_URL")
            .expect("VELO_INTEGRATION_DOWNLOAD_URL must be set explicitly");
        let format_id = std::env::var("VELO_INTEGRATION_DOWNLOAD_FORMAT")
            .expect("VELO_INTEGRATION_DOWNLOAD_FORMAT must be set explicitly");
        let destination = std::env::temp_dir().join(format!(
            "velo-download-integration-{}.mp4",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&destination);
        let _ = std::fs::remove_file(format!("{}.part", destination.to_string_lossy()));
        let task = DownloadTask::new(
            DownloadTaskId::new("integration-download").expect("task ID should be valid"),
            source_url,
            "Integration download",
            format_id,
            destination.to_string_lossy(),
            "mp4",
        )
        .expect("integration task should be valid");
        let (_cancellation, cancellation_receiver) = watch::channel(false);
        let (progress_sender, mut progress_receiver) = mpsc::unbounded_channel();
        let downloader = YtDlpDownloader::new(crate::infrastructure::configured_yt_dlp_path());

        let outcome = downloader
            .run(&task, cancellation_receiver, progress_sender)
            .await
            .expect("real download should succeed");

        assert_eq!(outcome, DownloadOutcome::Completed);
        assert!(destination.is_file());
        assert!(progress_receiver.try_recv().is_ok());
        std::fs::remove_file(destination).expect("integration output should be removable");
    }

    #[tokio::test]
    #[ignore = "requires an explicit public video URL, format ID, verified yt-dlp, and network"]
    async fn cancels_a_configured_real_download() {
        let source_url = std::env::var("VELO_INTEGRATION_DOWNLOAD_URL")
            .expect("VELO_INTEGRATION_DOWNLOAD_URL must be set explicitly");
        let format_id = std::env::var("VELO_INTEGRATION_DOWNLOAD_FORMAT")
            .expect("VELO_INTEGRATION_DOWNLOAD_FORMAT must be set explicitly");
        let destination = std::env::temp_dir().join(format!(
            "velo-download-cancel-integration-{}.mp4",
            std::process::id()
        ));
        let part_path = PathBuf::from(format!("{}.part", destination.to_string_lossy()));
        let _ = std::fs::remove_file(&destination);
        let _ = std::fs::remove_file(&part_path);
        let task = DownloadTask::new(
            DownloadTaskId::new("integration-cancel").expect("task ID should be valid"),
            source_url,
            "Integration cancellation",
            format_id,
            destination.to_string_lossy(),
            "mp4",
        )
        .expect("integration task should be valid");
        let (cancellation, cancellation_receiver) = watch::channel(false);
        let (progress_sender, mut progress_receiver) = mpsc::unbounded_channel();
        let downloader = YtDlpDownloader::new(crate::infrastructure::configured_yt_dlp_path());
        let download = downloader.run(&task, cancellation_receiver, progress_sender);
        tokio::pin!(download);

        let outcome = tokio::time::timeout(std::time::Duration::from_secs(30), async {
            tokio::select! {
                result = &mut download => result,
                progress = progress_receiver.recv() => {
                    progress.expect("download should emit progress before cancellation");
                    cancellation.send(true).expect("cancellation should be delivered");
                    download.await
                }
            }
        })
        .await
        .expect("cancellation should finish promptly")
        .expect("cancellation should not be a failure");

        assert_eq!(outcome, DownloadOutcome::Cancelled);
        assert!(!destination.exists());
        let _ = std::fs::remove_file(part_path);
    }
}
