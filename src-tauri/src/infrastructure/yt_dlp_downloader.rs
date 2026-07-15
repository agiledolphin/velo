use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
    process::Stdio,
};

use serde::Deserialize;
use tokio::{
    fs,
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, BufReader},
    process::Command,
    sync::{mpsc, watch},
    task::JoinHandle,
};

use crate::{
    application::{DownloadEngine, DownloadEngineUpdate, DownloadFuture, DownloadOutcome},
    domain::{DownloadFailure, DownloadProgress, DownloadTask},
};

use super::{YtDlpOptions, configured_deno_path};

const PROGRESS_PREFIX: &str = "VELO_PROGRESS:";
const PROCESSING_MARKER: &str = "VELO_PROCESSING";
const MAX_STDOUT_BYTES: usize = 32 * 1024 * 1024;
const MAX_STDERR_BYTES: usize = 1024 * 1024;
const MAX_LINE_BYTES: usize = 64 * 1024;

pub struct YtDlpDownloader {
    executable: PathBuf,
    ffmpeg: PathBuf,
    options: YtDlpOptions,
}

pub fn configured_ffmpeg_path() -> PathBuf {
    if let Some(path) = env::var_os("VELO_FFMPEG_PATH").map(PathBuf::from)
        && path.is_absolute()
    {
        return path;
    }

    let binary_name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    if let Ok(current_executable) = env::current_exe()
        && let Some(directory) = current_executable.parent()
    {
        let sibling = directory.join(binary_name);
        if sibling.is_file() {
            return sibling;
        }
    }

    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("binaries")
        .join(local_ffmpeg_sidecar_name())
}

fn local_ffmpeg_sidecar_name() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "ffmpeg-aarch64-apple-darwin",
        ("macos", "x86_64") => "ffmpeg-x86_64-apple-darwin",
        ("linux", "aarch64") => "ffmpeg-aarch64-unknown-linux-gnu",
        ("linux", "x86_64") => "ffmpeg-x86_64-unknown-linux-gnu",
        ("windows", "aarch64") => "ffmpeg-aarch64-pc-windows-msvc.exe",
        ("windows", "x86_64") => "ffmpeg-x86_64-pc-windows-msvc.exe",
        _ => binary_name_for_unsupported_target(),
    }
}

const fn binary_name_for_unsupported_target() -> &'static str {
    if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    }
}

impl YtDlpDownloader {
    pub fn new(executable: impl Into<PathBuf>, ffmpeg: impl Into<PathBuf>) -> Self {
        let executable = executable.into();
        let ffmpeg = ffmpeg.into();
        assert!(
            executable.is_absolute(),
            "download executable path must be absolute"
        );
        assert!(ffmpeg.is_absolute(), "FFmpeg path must be absolute");
        Self {
            executable,
            ffmpeg,
            options: YtDlpOptions::new(configured_deno_path()),
        }
    }

    pub fn with_options(
        executable: impl Into<PathBuf>,
        ffmpeg: impl Into<PathBuf>,
        options: YtDlpOptions,
    ) -> Self {
        let mut downloader = Self::new(executable, ffmpeg);
        downloader.options = options;
        downloader
    }
}

impl DownloadEngine for YtDlpDownloader {
    fn download<'a>(
        &'a self,
        task: &'a DownloadTask,
        cancellation: watch::Receiver<bool>,
        updates: mpsc::UnboundedSender<DownloadEngineUpdate>,
    ) -> DownloadFuture<'a> {
        Box::pin(async move { self.run(task, cancellation, updates).await })
    }
}

impl YtDlpDownloader {
    async fn run(
        &self,
        task: &DownloadTask,
        cancellation: watch::Receiver<bool>,
        updates: mpsc::UnboundedSender<DownloadEngineUpdate>,
    ) -> Result<DownloadOutcome, DownloadFailure> {
        let staging = DownloadStaging::create(task).await?;
        let result = self
            .run_staged(task, &staging.output, cancellation, updates)
            .await;
        match result {
            Ok(DownloadOutcome::Completed) => staging.commit().await,
            Ok(DownloadOutcome::Cancelled) => {
                staging.cleanup().await?;
                Ok(DownloadOutcome::Cancelled)
            }
            Err(error) => match staging.cleanup().await {
                Ok(()) => Err(error),
                Err(cleanup_error) => Err(cleanup_error),
            },
        }
    }

    async fn run_staged(
        &self,
        task: &DownloadTask,
        output_path: &Path,
        mut cancellation: watch::Receiver<bool>,
        updates: mpsc::UnboundedSender<DownloadEngineUpdate>,
    ) -> Result<DownloadOutcome, DownloadFailure> {
        let mut command = Command::new(&self.executable);
        command
            .args(download_arguments(
                task,
                output_path,
                &self.ffmpeg,
                &self.options,
            ))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = command
            .spawn()
            .map_err(|_| DownloadFailure::engine_unavailable())?;
        let stdout = child.stdout.take().ok_or_else(DownloadFailure::failed)?;
        let stderr = child.stderr.take().ok_or_else(DownloadFailure::failed)?;
        let stdout_task = tokio::spawn(read_progress(stdout, updates));
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
        let stderr = stderr_result?;

        if !status.success() {
            return Err(classify_download_failure(&stderr));
        }
        if !output_path.is_file() {
            return Err(DownloadFailure::failed());
        }

        Ok(DownloadOutcome::Completed)
    }
}

fn download_arguments(
    task: &DownloadTask,
    output_path: &Path,
    ffmpeg: &Path,
    options: &YtDlpOptions,
) -> Vec<OsString> {
    let format_selector = if task.has_video && !task.has_audio {
        format!("{}+bestaudio/{}", task.format_id, task.format_id)
    } else {
        task.format_id.clone()
    };
    let mut arguments = [
        "--ignore-config",
        "--no-plugin-dirs",
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
        "--progress-template",
        "postprocess:VELO_PROCESSING",
        "--ffmpeg-location",
    ]
    .into_iter()
    .map(OsString::from)
    .collect::<Vec<_>>();
    arguments.push(ffmpeg.as_os_str().to_owned());
    options.append_engine_arguments(
        &mut arguments,
        &task.source_url,
        options.should_use_cookie_for_media(&task.source_url),
    );
    for value in [
        "--merge-output-format",
        task.output_extension.as_str(),
        "--format",
        format_selector.as_str(),
        "--output",
    ] {
        arguments.push(OsString::from(value));
    }
    arguments.push(output_path.as_os_str().to_owned());
    arguments.push(OsString::from("--"));
    arguments.push(OsString::from(task.source_url.as_str()));
    arguments
}

async fn read_progress(
    reader: impl AsyncRead + Unpin,
    updates: mpsc::UnboundedSender<DownloadEngineUpdate>,
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

        if let Some(parsed) = parse_download_update(&String::from_utf8_lossy(&line)) {
            let _ = updates.send(parsed);
        }
    }

    if exceeded_limit {
        Err(DownloadFailure::output_too_large())
    } else {
        Ok(())
    }
}

async fn read_bounded(
    reader: impl AsyncRead + Unpin,
    limit: usize,
) -> Result<Vec<u8>, DownloadFailure> {
    let mut reader = reader;
    let mut buffer = [0u8; 8 * 1024];
    let mut output = Vec::new();
    let mut exceeded_limit = false;

    loop {
        let count = reader
            .read(&mut buffer)
            .await
            .map_err(|_| DownloadFailure::failed())?;
        if count == 0 {
            break;
        }
        if output.len().saturating_add(count) <= limit {
            output.extend_from_slice(&buffer[..count]);
        } else {
            exceeded_limit = true;
        }
    }

    if exceeded_limit {
        Err(DownloadFailure::output_too_large())
    } else {
        Ok(output)
    }
}

async fn stop_child(
    child: &mut tokio::process::Child,
    stdout_task: &JoinHandle<Result<(), DownloadFailure>>,
    stderr_task: &JoinHandle<Result<Vec<u8>, DownloadFailure>>,
) {
    let _ = child.kill().await;
    let _ = child.wait().await;
    stdout_task.abort();
    stderr_task.abort();
}

struct DownloadStaging {
    directory: PathBuf,
    output: PathBuf,
    destination: PathBuf,
}

impl DownloadStaging {
    async fn create(task: &DownloadTask) -> Result<Self, DownloadFailure> {
        let destination = PathBuf::from(&task.destination_path);
        let parent = destination
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .ok_or_else(DownloadFailure::destination_unavailable)?;
        let file_name = destination
            .file_name()
            .ok_or_else(DownloadFailure::destination_unavailable)?;
        let directory = parent.join(format!(".velo-{}", task.id.as_str()));
        fs::create_dir(&directory)
            .await
            .map_err(map_destination_error)?;
        let output = directory.join(file_name);
        Ok(Self {
            directory,
            output,
            destination,
        })
    }

    async fn commit(self) -> Result<DownloadOutcome, DownloadFailure> {
        let backup = self.directory.join("previous-download");
        let mut has_backup = false;
        if let Ok(metadata) = fs::metadata(&self.destination).await {
            if !metadata.is_file() {
                self.cleanup().await?;
                return Err(DownloadFailure::destination_unavailable());
            }
            fs::rename(&self.destination, &backup)
                .await
                .map_err(map_destination_error)?;
            has_backup = true;
        }

        if let Err(error) = fs::rename(&self.output, &self.destination).await {
            if has_backup {
                let _ = fs::rename(&backup, &self.destination).await;
            }
            let _ = self.cleanup().await;
            return Err(map_destination_error(error));
        }
        if has_backup {
            fs::remove_file(&backup)
                .await
                .map_err(|_| DownloadFailure::cleanup_failed())?;
        }
        fs::remove_dir(&self.directory)
            .await
            .map_err(|_| DownloadFailure::cleanup_failed())?;
        Ok(DownloadOutcome::Completed)
    }

    async fn cleanup(&self) -> Result<(), DownloadFailure> {
        match fs::remove_dir_all(&self.directory).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(DownloadFailure::cleanup_failed()),
        }
    }
}

fn classify_download_failure(stderr: &[u8]) -> DownloadFailure {
    let message = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    if contains_any(
        &message,
        &["no space left", "disk full", "not enough space"],
    ) {
        DownloadFailure::disk_full()
    } else if contains_any(
        &message,
        &["permission denied", "access is denied", "access denied"],
    ) {
        DownloadFailure::permission_denied()
    } else {
        DownloadFailure::failed()
    }
}

fn map_destination_error(error: std::io::Error) -> DownloadFailure {
    if matches!(error.raw_os_error(), Some(28 | 39 | 112)) {
        DownloadFailure::disk_full()
    } else if error.kind() == std::io::ErrorKind::PermissionDenied {
        DownloadFailure::permission_denied()
    } else {
        DownloadFailure::destination_unavailable()
    }
}

fn contains_any(message: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|pattern| message.contains(pattern))
}

#[derive(Deserialize)]
struct RawProgress {
    downloaded_bytes: Option<f64>,
    total_bytes: Option<f64>,
    total_bytes_estimate: Option<f64>,
    speed: Option<f64>,
    eta: Option<f64>,
}

fn parse_download_update(line: &str) -> Option<DownloadEngineUpdate> {
    let line = line.trim();
    if line == PROCESSING_MARKER {
        return Some(DownloadEngineUpdate::Processing);
    }
    let payload = line.strip_prefix(PROGRESS_PREFIX)?;
    let raw: RawProgress = serde_json::from_str(payload).ok()?;
    Some(DownloadEngineUpdate::Progress(DownloadProgress {
        downloaded_bytes: finite_u64(raw.downloaded_bytes).unwrap_or(0),
        total_bytes: finite_u64(raw.total_bytes.or(raw.total_bytes_estimate)),
        speed_bytes_per_second: finite_u64(raw.speed),
        eta_seconds: finite_u64(raw.eta),
    }))
}

fn finite_u64(value: Option<f64>) -> Option<u64> {
    value
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| value.round() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DownloadStreams, DownloadTaskId};

    fn task() -> DownloadTask {
        DownloadTask::new(
            DownloadTaskId::new("task-1").expect("task ID should be valid"),
            "https://video.example/watch?v=1&unsafe=true",
            "Title",
            "137/mp4",
            std::env::temp_dir().join("video.mp4").to_string_lossy(),
            "mp4",
            DownloadStreams::VideoOnly,
        )
        .expect("task should be valid")
    }

    #[test]
    fn uses_a_fixed_download_argument_contract() {
        let arguments = download_arguments(
            &task(),
            Path::new("/trusted/staging/video.mp4"),
            Path::new("/trusted/ffmpeg"),
            &YtDlpOptions::new(configured_deno_path()),
        );
        let strings = arguments
            .iter()
            .map(|value| value.to_string_lossy())
            .collect::<Vec<_>>();

        assert!(strings.iter().any(|value| value == "--ignore-config"));
        assert!(strings.iter().any(|value| value == "--no-overwrites"));
        assert!(strings.iter().any(|value| value == "--no-exec"));
        assert!(strings.iter().any(|value| value == "/trusted/ffmpeg"));
        assert!(
            strings
                .iter()
                .any(|value| value == "/trusted/staging/video.mp4")
        );
        assert!(
            strings
                .iter()
                .any(|value| value == "137/mp4+bestaudio/137/mp4")
        );
        assert!(strings.iter().any(|value| value == "mp4"));
        assert_eq!(strings[strings.len() - 2], "--");
        assert_eq!(
            strings.last().expect("URL"),
            "https://video.example/watch?v=1&unsafe=true"
        );
    }

    #[test]
    fn keeps_combined_formats_as_a_single_selection() {
        let mut task = task();
        task.has_audio = true;
        let arguments = download_arguments(
            &task,
            Path::new("/trusted/staging/video.mp4"),
            Path::new("/trusted/ffmpeg"),
            &YtDlpOptions::new(configured_deno_path()),
        );
        let strings = arguments
            .iter()
            .map(|value| value.to_string_lossy())
            .collect::<Vec<_>>();

        assert!(strings.iter().any(|value| value == "137/mp4"));
        assert!(!strings.iter().any(|value| value.contains("+bestaudio")));
    }

    #[test]
    fn parses_machine_readable_progress() {
        let update = parse_download_update(
            r#"VELO_PROGRESS:{"downloaded_bytes":512,"total_bytes":1024,"total_bytes_estimate":null,"speed":256.4,"eta":2}"#,
        )
        .expect("progress should parse");
        let DownloadEngineUpdate::Progress(progress) = update else {
            panic!("expected a progress update");
        };

        assert_eq!(progress.downloaded_bytes, 512);
        assert_eq!(progress.total_bytes, Some(1024));
        assert_eq!(progress.speed_bytes_per_second, Some(256));
        assert_eq!(progress.eta_seconds, Some(2));
        assert!(parse_download_update("WARNING: ignored").is_none());
        assert_eq!(
            parse_download_update("VELO_PROCESSING"),
            Some(DownloadEngineUpdate::Processing)
        );
    }

    #[test]
    fn classifies_expected_storage_failures() {
        assert_eq!(
            classify_download_failure(b"ERROR: No space left on device").code,
            "download_disk_full"
        );
        assert_eq!(
            classify_download_failure(b"ERROR: Permission denied").code,
            "download_permission_denied"
        );
        assert_eq!(
            classify_download_failure(b"ERROR: unavailable").code,
            "download_failed"
        );
    }

    #[tokio::test]
    async fn stages_then_replaces_an_existing_destination() {
        let root = unique_test_directory("replace");
        fs::create_dir(&root)
            .await
            .expect("test directory should be created");
        let destination = root.join("video.mp4");
        fs::write(&destination, b"old")
            .await
            .expect("existing file should be written");
        let task = task_at(&destination, "replace-task");
        let staging = DownloadStaging::create(&task)
            .await
            .expect("staging should be created");
        fs::write(&staging.output, b"new")
            .await
            .expect("staged file should be written");

        assert_eq!(
            fs::read(&destination).await.expect("old file remains"),
            b"old"
        );
        assert_eq!(
            staging.commit().await.expect("commit should succeed"),
            DownloadOutcome::Completed
        );
        assert_eq!(
            fs::read(&destination).await.expect("new file exists"),
            b"new"
        );
        fs::remove_dir_all(root)
            .await
            .expect("test directory should be removed");
    }

    #[tokio::test]
    async fn removes_the_whole_staging_directory_after_cancellation() {
        let root = unique_test_directory("cancel");
        fs::create_dir(&root)
            .await
            .expect("test directory should be created");
        let task = task_at(&root.join("video.mp4"), "cancel-task");
        let staging = DownloadStaging::create(&task)
            .await
            .expect("staging should be created");
        fs::write(staging.directory.join("video.mp4.part"), b"partial")
            .await
            .expect("partial file should be written");
        staging.cleanup().await.expect("cleanup should succeed");
        assert!(!staging.directory.exists());
        fs::remove_dir_all(root)
            .await
            .expect("test directory should be removed");
    }

    fn task_at(destination: &Path, id: &str) -> DownloadTask {
        DownloadTask::new(
            DownloadTaskId::new(id).expect("task ID should be valid"),
            "https://video.example/watch?v=1",
            "Title",
            "137/mp4",
            destination.to_string_lossy(),
            "mp4",
            DownloadStreams::VideoOnly,
        )
        .expect("task should be valid")
    }

    fn unique_test_directory(label: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time should be valid")
            .as_nanos();
        std::env::temp_dir().join(format!("velo-{label}-{}-{nonce}", std::process::id()))
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
            DownloadStreams::VideoOnly,
        )
        .expect("integration task should be valid");
        let (_cancellation, cancellation_receiver) = watch::channel(false);
        let (progress_sender, mut progress_receiver) = mpsc::unbounded_channel();
        let downloader = YtDlpDownloader::new(
            crate::infrastructure::configured_yt_dlp_path(),
            configured_ffmpeg_path(),
        );

        let outcome = downloader
            .run(&task, cancellation_receiver, progress_sender)
            .await
            .expect("real download should succeed");

        assert_eq!(outcome, DownloadOutcome::Completed);
        assert!(destination.is_file());
        assert!(progress_receiver.try_recv().is_ok());
        let media_check = Command::new(configured_ffmpeg_path())
            .args([
                "-v",
                "error",
                "-i",
                destination.to_string_lossy().as_ref(),
                "-map",
                "0:v:0",
                "-map",
                "0:a:0",
                "-t",
                "0.1",
                "-f",
                "null",
                "-",
            ])
            .status()
            .await
            .expect("FFmpeg should inspect the downloaded media");
        assert!(
            media_check.success(),
            "output should contain video and audio"
        );
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
            DownloadStreams::VideoOnly,
        )
        .expect("integration task should be valid");
        let (cancellation, cancellation_receiver) = watch::channel(false);
        let (progress_sender, mut progress_receiver) = mpsc::unbounded_channel();
        let downloader = YtDlpDownloader::new(
            crate::infrastructure::configured_yt_dlp_path(),
            configured_ffmpeg_path(),
        );
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
