mod application;
mod commands;
pub mod domain;
mod infrastructure;

use std::time::Duration;

use application::{AppState, DownloadCoordinator};
use infrastructure::{
    RestrictedProcessRunner, ThumbnailFetcher, YtDlpDownloader, YtDlpEngine,
    configured_ffmpeg_path, configured_yt_dlp_path,
};

const INSPECT_TIMEOUT: Duration = Duration::from_secs(45);
const MAX_ENGINE_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let engine_path = configured_yt_dlp_path();
    let ffmpeg_path = configured_ffmpeg_path();
    let runner = RestrictedProcessRunner::new(
        engine_path.clone(),
        INSPECT_TIMEOUT,
        MAX_ENGINE_OUTPUT_BYTES,
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new(YtDlpEngine::new(runner)))
        .manage(DownloadCoordinator::new(YtDlpDownloader::new(
            engine_path,
            ffmpeg_path,
        )))
        .manage(ThumbnailFetcher)
        .invoke_handler(tauri::generate_handler![
            commands::download::suggest_download_file_name,
            commands::download::prepare_download_task,
            commands::download::start_download,
            commands::download::cancel_download,
            commands::media::inspect_url,
            commands::media::cancel_inspection,
            commands::thumbnail::fetch_thumbnail
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Velo");
}
