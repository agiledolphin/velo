use serde::Serialize;

use crate::domain::{DownloadModelError, DownloadTask, DownloadTaskId, suggested_file_name};

#[derive(Debug, Serialize)]
pub struct PrepareDownloadError {
    code: &'static str,
    message: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadFileSuggestion {
    file_name: String,
    extension: String,
}

impl From<DownloadModelError> for PrepareDownloadError {
    fn from(error: DownloadModelError) -> Self {
        match error {
            DownloadModelError::DestinationExtension => Self {
                code: "destination_extension_mismatch",
                message: "保存文件的扩展名与所选格式不一致。",
            },
            DownloadModelError::DestinationPath => Self {
                code: "invalid_destination_path",
                message: "请选择有效的绝对保存路径。",
            },
            DownloadModelError::ExpectedExtension => Self {
                code: "invalid_format",
                message: "所选媒体格式无效，请重新解析后再试。",
            },
            DownloadModelError::TaskId
            | DownloadModelError::SourceUrl
            | DownloadModelError::MediaTitle
            | DownloadModelError::FormatId => Self {
                code: "invalid_download_request",
                message: "下载请求无效，请重新解析后再试。",
            },
        }
    }
}

#[tauri::command]
pub fn suggest_download_file_name(title: String, extension: String) -> DownloadFileSuggestion {
    let file_name = suggested_file_name(&title, &extension);
    let extension = file_name
        .rsplit_once('.')
        .map(|(_, extension)| extension)
        .unwrap_or("mp4")
        .to_owned();
    DownloadFileSuggestion {
        file_name,
        extension,
    }
}

#[tauri::command]
pub fn prepare_download_task(
    task_id: String,
    source_url: String,
    media_title: String,
    format_id: String,
    destination_path: String,
    expected_extension: String,
) -> Result<DownloadTask, PrepareDownloadError> {
    let task_id = DownloadTaskId::new(task_id)?;
    DownloadTask::new(
        task_id,
        source_url,
        media_title,
        format_id,
        destination_path,
        &expected_extension,
    )
    .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn does_not_expose_invalid_destination_details() {
        let error = prepare_download_task(
            "task-1".into(),
            "https://video.example/watch".into(),
            "Title".into(),
            "format".into(),
            "relative/private/path.mp4".into(),
            "mp4".into(),
        )
        .expect_err("relative path should fail");

        assert_eq!(error.code, "invalid_destination_path");
        assert!(!error.message.contains("private"));
    }
}
