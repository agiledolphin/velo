use serde::Serialize;
use url::Url;

const MAX_TASK_ID_LENGTH: usize = 64;
const MAX_MEDIA_TITLE_LENGTH: usize = 512;
const MAX_FORMAT_ID_LENGTH: usize = 128;

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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTask {
    pub id: DownloadTaskId,
    pub source_url: String,
    pub media_title: String,
    pub format_id: String,
}

impl DownloadTask {
    pub fn new(
        id: DownloadTaskId,
        source_url: impl Into<String>,
        media_title: impl Into<String>,
        format_id: impl Into<String>,
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

        Ok(Self {
            id,
            source_url,
            media_title,
            format_id,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DownloadModelError {
    TaskId,
    SourceUrl,
    MediaTitle,
    FormatId,
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
        )
        .expect("download task should be valid");

        assert_eq!(task.source_url, "https://video.example/watch/1");
        assert_eq!(task.format_id, "1080p-mp4");
    }

    #[test]
    fn rejects_invalid_download_task_fields() {
        assert_eq!(
            DownloadTaskId::new("invalid task id"),
            Err(DownloadModelError::TaskId)
        );
        assert_eq!(
            DownloadTask::new(task_id(), "file:///tmp/video", "Title", "format"),
            Err(DownloadModelError::SourceUrl)
        );
        assert_eq!(
            DownloadTask::new(task_id(), "https://video.example/1", " ", "format"),
            Err(DownloadModelError::MediaTitle)
        );
        assert_eq!(
            DownloadTask::new(task_id(), "https://video.example/1", "Title", " "),
            Err(DownloadModelError::FormatId)
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
