mod application;
mod commands;
pub mod domain;
mod infrastructure;

use std::time::Duration;

use application::{AppState, DownloadCoordinator};
use infrastructure::{
    RepresentativeFrameGenerator, RestrictedProcessRunner, ThumbnailFetcher, YtDlpDownloader,
    YtDlpEngine, configured_ffmpeg_path, configured_yt_dlp_path,
};

const INSPECT_TIMEOUT: Duration = Duration::from_secs(45);
const MAX_ENGINE_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
const FRAME_TIMEOUT: Duration = Duration::from_secs(45);
const MAX_STREAM_OUTPUT_BYTES: usize = 256 * 1024;
const MAX_FRAME_OUTPUT_BYTES: usize = 5 * 1024 * 1024;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let engine_path = configured_yt_dlp_path();
    let ffmpeg_path = configured_ffmpeg_path();
    let runner = RestrictedProcessRunner::new(
        engine_path.clone(),
        INSPECT_TIMEOUT,
        MAX_ENGINE_OUTPUT_BYTES,
    );
    let frame_generator = RepresentativeFrameGenerator::new(
        RestrictedProcessRunner::new(engine_path.clone(), FRAME_TIMEOUT, MAX_STREAM_OUTPUT_BYTES),
        RestrictedProcessRunner::new(ffmpeg_path.clone(), FRAME_TIMEOUT, MAX_FRAME_OUTPUT_BYTES),
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new(YtDlpEngine::new(runner)))
        .manage(DownloadCoordinator::new(YtDlpDownloader::new(
            engine_path,
            ffmpeg_path,
        )))
        .manage(ThumbnailFetcher)
        .manage(frame_generator)
        .invoke_handler(tauri::generate_handler![
            commands::download::suggest_download_file_name,
            commands::download::prepare_download_task,
            commands::download::start_download,
            commands::download::cancel_download,
            commands::media::inspect_url,
            commands::media::cancel_inspection,
            commands::thumbnail::fetch_thumbnail,
            commands::thumbnail::generate_representative_frame
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Velo");
}
