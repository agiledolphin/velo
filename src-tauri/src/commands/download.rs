use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::{
    application::{DownloadCoordinator, StartDownloadError},
    domain::{
        DownloadModelError, DownloadStreams, DownloadTask, DownloadTaskId, suggested_file_name,
    },
    infrastructure::YtDlpOptions,
};

const DOWNLOAD_EVENT_NAME: &str = "download-event";

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
            | DownloadModelError::FormatId
            | DownloadModelError::StreamFlags => Self {
                code: "invalid_download_request",
                message: "下载请求无效，请重新解析后再试。",
            },
        }
    }
}

impl From<StartDownloadError> for PrepareDownloadError {
    fn from(error: StartDownloadError) -> Self {
        match error {
            StartDownloadError::AlreadyRunning => Self {
                code: "download_already_running",
                message: "这个下载任务已经在运行。",
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
#[allow(clippy::too_many_arguments)]
pub fn prepare_download_task(
    task_id: String,
    source_url: String,
    media_title: String,
    format_id: String,
    destination_path: String,
    expected_extension: String,
    has_video: bool,
    has_audio: bool,
) -> Result<DownloadTask, PrepareDownloadError> {
    let task_id = DownloadTaskId::new(task_id)?;
    let streams = DownloadStreams::from_flags(has_video, has_audio)?;
    DownloadTask::new(
        task_id,
        source_url,
        media_title,
        format_id,
        destination_path,
        &expected_extension,
        streams,
    )
    .map_err(Into::into)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn prepare_default_download_task(
    task_id: String,
    source_url: String,
    media_title: String,
    format_id: String,
    expected_extension: String,
    has_video: bool,
    has_audio: bool,
    options: State<'_, YtDlpOptions>,
) -> Result<DownloadTask, PrepareDownloadError> {
    let directory = options
        .download_directory()
        .map_err(|_| PrepareDownloadError {
            code: "download_directory_unavailable",
            message: "默认下载目录不可用，请在设置中重新选择。",
        })?;
    let file_name = suggested_file_name(&media_title, &expected_extension);
    let destination =
        available_destination(&directory, &file_name).ok_or(PrepareDownloadError {
            code: "destination_unavailable",
            message: "无法生成可用的目标文件名，请使用另存为。",
        })?;
    prepare_download_task(
        task_id,
        source_url,
        media_title,
        format_id,
        destination.to_string_lossy().into_owned(),
        expected_extension,
        has_video,
        has_audio,
    )
}

fn available_destination(directory: &Path, file_name: &str) -> Option<PathBuf> {
    let original = directory.join(file_name);
    if !original.exists() {
        return Some(original);
    }
    let path = Path::new(file_name);
    let stem = path.file_stem()?.to_string_lossy();
    let extension = path.extension()?.to_string_lossy();
    (1..=9_999)
        .map(|index| directory.join(format!("{stem} ({index}).{extension}")))
        .find(|candidate| !candidate.exists())
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn start_download(
    task_id: String,
    source_url: String,
    media_title: String,
    format_id: String,
    destination_path: String,
    expected_extension: String,
    has_video: bool,
    has_audio: bool,
    app: AppHandle,
    coordinator: State<'_, DownloadCoordinator>,
) -> Result<DownloadTask, PrepareDownloadError> {
    let streams = DownloadStreams::from_flags(has_video, has_audio)?;
    let task = DownloadTask::new(
        DownloadTaskId::new(task_id)?,
        source_url,
        media_title,
        format_id,
        destination_path,
        &expected_extension,
        streams,
    )?;
    if Path::new(&task.destination_path).exists() {
        return Err(PrepareDownloadError {
            code: "destination_exists",
            message: "目标文件已存在，请重新选择保存位置。",
        });
    }

    let run = coordinator.begin(&task.id)?;
    let coordinator = (*coordinator).clone();
    let running_task = task.clone();
    tauri::async_runtime::spawn(async move {
        coordinator
            .run(running_task, run, |event| {
                let _ = app.emit(DOWNLOAD_EVENT_NAME, event);
            })
            .await;
    });

    Ok(task)
}

#[tauri::command]
pub fn cancel_download(task_id: String, coordinator: State<'_, DownloadCoordinator>) -> bool {
    DownloadTaskId::new(task_id.as_str()).is_ok() && coordinator.cancel(&task_id)
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
            true,
            false,
        )
        .expect_err("relative path should fail");

        assert_eq!(error.code, "invalid_destination_path");
        assert!(!error.message.contains("private"));
    }

    #[test]
    fn adds_a_number_without_overwriting_an_existing_file() {
        let root = std::env::temp_dir().join(format!(
            "velo-destination-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be valid")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("fixture directory should exist");
        std::fs::write(root.join("Video.mp4"), b"existing").expect("fixture should exist");

        assert_eq!(
            available_destination(&root, "Video.mp4"),
            Some(root.join("Video (1).mp4"))
        );

        std::fs::remove_dir_all(root).expect("fixture should be removed");
    }
}
