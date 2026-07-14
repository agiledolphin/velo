use std::path::Path;

use serde::Serialize;
use url::Url;

const MAX_TASK_ID_LENGTH: usize = 64;
const MAX_MEDIA_TITLE_LENGTH: usize = 512;
const MAX_FORMAT_ID_LENGTH: usize = 128;
const MAX_FILE_STEM_CHARS: usize = 120;
const MAX_FILE_NAME_BYTES: usize = 255;
const MAX_EXTENSION_LENGTH: usize = 10;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct DownloadTaskId(String);

impl DownloadTaskId {
    pub fn new(value: impl Into<String>) -> Result<Self, DownloadModelError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_TASK_ID_LENGTH
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err(DownloadModelError::TaskId);
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTask {
    pub id: DownloadTaskId,
    pub source_url: String,
    pub media_title: String,
    pub format_id: String,
    pub destination_path: String,
    pub output_extension: String,
    pub has_video: bool,
    pub has_audio: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadStreams {
    AudioVideo,
    VideoOnly,
    AudioOnly,
}

impl DownloadStreams {
    pub fn from_flags(has_video: bool, has_audio: bool) -> Result<Self, DownloadModelError> {
        match (has_video, has_audio) {
            (true, true) => Ok(Self::AudioVideo),
            (true, false) => Ok(Self::VideoOnly),
            (false, true) => Ok(Self::AudioOnly),
            (false, false) => Err(DownloadModelError::StreamFlags),
        }
    }

    fn flags(self) -> (bool, bool) {
        match self {
            Self::AudioVideo => (true, true),
            Self::VideoOnly => (true, false),
            Self::AudioOnly => (false, true),
        }
    }
}

impl DownloadTask {
    pub fn new(
        id: DownloadTaskId,
        source_url: impl Into<String>,
        media_title: impl Into<String>,
        format_id: impl Into<String>,
        destination_path: impl Into<String>,
        expected_extension: &str,
        streams: DownloadStreams,
    ) -> Result<Self, DownloadModelError> {
        let source_url = source_url.into();
        let parsed_url = Url::parse(&source_url).map_err(|_| DownloadModelError::SourceUrl)?;
        if !matches!(parsed_url.scheme(), "http" | "https") {
            return Err(DownloadModelError::SourceUrl);
        }

        let media_title = media_title.into();
        if media_title.trim().is_empty() || media_title.len() > MAX_MEDIA_TITLE_LENGTH {
            return Err(DownloadModelError::MediaTitle);
        }

        let format_id = format_id.into();
        if format_id.trim().is_empty() || format_id.len() > MAX_FORMAT_ID_LENGTH {
            return Err(DownloadModelError::FormatId);
        }

        let output_extension =
            normalize_extension(expected_extension).ok_or(DownloadModelError::ExpectedExtension)?;
        let destination_path = destination_path.into();
        validate_destination_path(&destination_path, &output_extension)?;
        let (has_video, has_audio) = streams.flags();

        Ok(Self {
            id,
            source_url,
            media_title,
            format_id,
            destination_path,
            output_extension,
            has_video,
            has_audio,
        })
    }
}

pub fn suggested_file_name(title: &str, extension: &str) -> String {
    let extension = normalize_extension(extension).unwrap_or_else(|| "mp4".into());
    let mut stem = title
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                )
            {
                ' '
            } else {
                character
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches([' ', '.'])
        .chars()
        .take(MAX_FILE_STEM_CHARS)
        .collect::<String>();

    stem = stem.trim_end_matches([' ', '.']).to_owned();
    if stem.is_empty() {
        stem = "video".into();
    }
    let max_stem_bytes = MAX_FILE_NAME_BYTES - extension.len() - 1;
    while stem.len() > max_stem_bytes {
        stem.pop();
    }
    stem = stem.trim_end_matches([' ', '.']).to_owned();
    if is_windows_reserved_name(&stem) {
        stem.insert(0, '_');
    }

    format!("{stem}.{extension}")
}

fn validate_destination_path(
    destination_path: &str,
    expected_extension: &str,
) -> Result<(), DownloadModelError> {
    let expected_extension =
        normalize_extension(expected_extension).ok_or(DownloadModelError::ExpectedExtension)?;
    let path = Path::new(destination_path);
    if destination_path.trim().is_empty() || !path.is_absolute() {
        return Err(DownloadModelError::DestinationPath);
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(DownloadModelError::DestinationPath)?;
    if file_name.is_empty()
        || file_name.len() > MAX_FILE_NAME_BYTES
        || file_name.contains('\0')
        || file_name.trim_matches([' ', '.']).is_empty()
    {
        return Err(DownloadModelError::DestinationPath);
    }

    let actual_extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    if actual_extension.as_deref() != Some(expected_extension.as_str()) {
        return Err(DownloadModelError::DestinationExtension);
    }

    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or(DownloadModelError::DestinationPath)?;
    if is_windows_reserved_name(stem) {
        return Err(DownloadModelError::DestinationPath);
    }

    Ok(())
}

fn normalize_extension(extension: &str) -> Option<String> {
    let extension = extension.trim().trim_start_matches('.');
    (!extension.is_empty()
        && extension.len() <= MAX_EXTENSION_LENGTH
        && extension.bytes().all(|byte| byte.is_ascii_alphanumeric()))
    .then(|| extension.to_ascii_lowercase())
}

fn is_windows_reserved_name(stem: &str) -> bool {
    let name = stem.split('.').next().unwrap_or(stem).to_ascii_uppercase();
    matches!(name.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || name
            .strip_prefix("COM")
            .or_else(|| name.strip_prefix("LPT"))
            .is_some_and(|suffix| {
                matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DownloadModelError {
    TaskId,
    SourceUrl,
    MediaTitle,
    FormatId,
    DestinationPath,
    DestinationExtension,
    ExpectedExtension,
    StreamFlags,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub speed_bytes_per_second: Option<u64>,
    pub eta_seconds: Option<u64>,
}

impl DownloadProgress {
    pub fn fraction(&self) -> Option<f64> {
        self.total_bytes
            .filter(|total| *total > 0)
            .map(|total| (self.downloaded_bytes as f64 / total as f64).min(1.0))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadFailure {
    pub code: String,
    pub message: String,
}

impl DownloadFailure {
    pub fn engine_unavailable() -> Self {
        Self {
            code: "download_engine_unavailable".into(),
            message: "未找到下载引擎，请重新安装 yt-dlp。".into(),
        }
    }

    pub fn failed() -> Self {
        Self {
            code: "download_failed".into(),
            message: "下载失败，请确认页面仍可访问后重试。".into(),
        }
    }

    pub fn output_too_large() -> Self {
        Self {
            code: "download_output_too_large".into(),
            message: "下载引擎返回了过多诊断信息，任务已停止。".into(),
        }
    }

    pub fn disk_full() -> Self {
        Self {
            code: "download_disk_full".into(),
            message: "保存磁盘空间不足，请清理空间或选择其他位置。".into(),
        }
    }

    pub fn permission_denied() -> Self {
        Self {
            code: "download_permission_denied".into(),
            message: "没有权限写入所选位置，请选择其他文件夹。".into(),
        }
    }

    pub fn destination_unavailable() -> Self {
        Self {
            code: "download_destination_unavailable".into(),
            message: "无法写入或替换目标文件，请确认文件未被其他程序占用。".into(),
        }
    }

    pub fn cleanup_failed() -> Self {
        Self {
            code: "download_cleanup_failed".into(),
            message: "任务已停止，但临时文件清理失败，请检查保存目录。".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadEvent {
    pub task_id: DownloadTaskId,
    pub sequence: u64,
    #[serde(flatten)]
    pub payload: DownloadEventPayload,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DownloadEventPayload {
    Queued,
    Started,
    Progress { progress: DownloadProgress },
    Processing,
    Completed,
    Cancelled,
    Failed { error: DownloadFailure },
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn task_id() -> DownloadTaskId {
        DownloadTaskId::new("download_01-test").expect("task ID should be valid")
    }

    #[test]
    fn creates_a_valid_download_task() {
        let task = DownloadTask::new(
            task_id(),
            "https://video.example/watch/1",
            "Example video",
            "1080p-mp4",
            std::env::temp_dir()
                .join("Example video.mp4")
                .to_string_lossy(),
            "mp4",
            DownloadStreams::VideoOnly,
        )
        .expect("download task should be valid");

        assert_eq!(task.source_url, "https://video.example/watch/1");
        assert_eq!(task.format_id, "1080p-mp4");
        assert_eq!(task.output_extension, "mp4");
        assert!(task.has_video);
        assert!(!task.has_audio);
        assert!(task.destination_path.ends_with("Example video.mp4"));
    }

    #[test]
    fn rejects_invalid_download_task_fields() {
        assert_eq!(
            DownloadTaskId::new("invalid task id"),
            Err(DownloadModelError::TaskId)
        );
        assert_eq!(
            DownloadTask::new(
                task_id(),
                "file:///tmp/video",
                "Title",
                "format",
                std::env::temp_dir().join("video.mp4").to_string_lossy(),
                "mp4",
                DownloadStreams::VideoOnly,
            ),
            Err(DownloadModelError::SourceUrl)
        );
        assert_eq!(
            DownloadTask::new(
                task_id(),
                "https://video.example/1",
                " ",
                "format",
                std::env::temp_dir().join("video.mp4").to_string_lossy(),
                "mp4",
                DownloadStreams::VideoOnly,
            ),
            Err(DownloadModelError::MediaTitle)
        );
        assert_eq!(
            DownloadTask::new(
                task_id(),
                "https://video.example/1",
                "Title",
                " ",
                std::env::temp_dir().join("video.mp4").to_string_lossy(),
                "mp4",
                DownloadStreams::VideoOnly,
            ),
            Err(DownloadModelError::FormatId)
        );
        assert_eq!(
            DownloadStreams::from_flags(false, false),
            Err(DownloadModelError::StreamFlags)
        );
    }

    #[test]
    fn creates_portable_suggested_file_names() {
        assert_eq!(
            suggested_file_name("  微落：轻取/流光？  ", ".MP4"),
            "微落：轻取 流光？.mp4"
        );
        assert_eq!(suggested_file_name("CON", "mp4"), "_CON.mp4");
        assert_eq!(suggested_file_name("...", "bad/ext"), "video.mp4");
        assert!(suggested_file_name(&"微".repeat(120), "webm").len() <= 255);
    }

    #[test]
    fn rejects_unsafe_or_mismatched_destinations() {
        let base = std::env::temp_dir();
        let reserved = base.join("CON.mp4").to_string_lossy().into_owned();
        let wrong_extension = base.join("video.webm").to_string_lossy().into_owned();

        assert_eq!(
            DownloadTask::new(
                task_id(),
                "https://video.example/1",
                "Title",
                "format",
                "video.mp4",
                "mp4",
                DownloadStreams::VideoOnly,
            ),
            Err(DownloadModelError::DestinationPath)
        );
        assert_eq!(
            DownloadTask::new(
                task_id(),
                "https://video.example/1",
                "Title",
                "format",
                reserved,
                "mp4",
                DownloadStreams::VideoOnly,
            ),
            Err(DownloadModelError::DestinationPath)
        );
        assert_eq!(
            DownloadTask::new(
                task_id(),
                "https://video.example/1",
                "Title",
                "format",
                wrong_extension,
                "mp4",
                DownloadStreams::VideoOnly,
            ),
            Err(DownloadModelError::DestinationExtension)
        );
    }

    #[test]
    fn serializes_progress_events_for_the_frontend_contract() {
        let event = DownloadEvent {
            task_id: task_id(),
            sequence: 3,
            payload: DownloadEventPayload::Progress {
                progress: DownloadProgress {
                    downloaded_bytes: 512,
                    total_bytes: Some(1024),
                    speed_bytes_per_second: Some(256),
                    eta_seconds: Some(2),
                },
            },
        };

        assert_eq!(
            serde_json::to_value(event).expect("event should serialize"),
            json!({
                "taskId": "download_01-test",
                "sequence": 3,
                "type": "progress",
                "progress": {
                    "downloadedBytes": 512,
                    "totalBytes": 1024,
                    "speedBytesPerSecond": 256,
                    "etaSeconds": 2
                }
            })
        );
    }

    #[test]
    fn serializes_every_lifecycle_event_with_a_stable_type() {
        let payloads = [
            DownloadEventPayload::Queued,
            DownloadEventPayload::Started,
            DownloadEventPayload::Processing,
            DownloadEventPayload::Completed,
            DownloadEventPayload::Cancelled,
            DownloadEventPayload::Failed {
                error: DownloadFailure {
                    code: "download_failed".into(),
                    message: "下载失败，请稍后重试。".into(),
                },
            },
        ];
        let expected_types = [
            "queued",
            "started",
            "processing",
            "completed",
            "cancelled",
            "failed",
        ];

        for (payload, expected_type) in payloads.into_iter().zip(expected_types) {
            let event = DownloadEvent {
                task_id: task_id(),
                sequence: 1,
                payload,
            };
            let value = serde_json::to_value(event).expect("event should serialize");
            assert_eq!(value["type"], expected_type);
        }
    }

    #[test]
    fn calculates_bounded_progress_only_with_a_known_total() {
        assert_eq!(DownloadProgress::default().fraction(), None);
        assert_eq!(
            DownloadProgress {
                downloaded_bytes: 150,
                total_bytes: Some(100),
                ..DownloadProgress::default()
            }
            .fraction(),
            Some(1.0)
        );
    }
}
