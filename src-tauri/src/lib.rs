mod application;
mod commands;
pub mod domain;
mod infrastructure;

use std::time::Duration;

use application::{AppState, DownloadCoordinator};
use infrastructure::{
    RepresentativeFrameCache, RepresentativeFrameGenerator, RestrictedProcessRunner,
    ThumbnailFetcher, YtDlpDownloader, YtDlpEngine, YtDlpOptions, configured_deno_path,
    configured_ffmpeg_path, configured_yt_dlp_path,
};
use tauri::Manager;

const INSPECT_TIMEOUT: Duration = Duration::from_secs(45);
const MAX_ENGINE_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
const FRAME_TIMEOUT: Duration = Duration::from_secs(45);
const MAX_STREAM_OUTPUT_BYTES: usize = 256 * 1024;
const MAX_FRAME_OUTPUT_BYTES: usize = 5 * 1024 * 1024;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let engine_path = configured_yt_dlp_path();
            let ffmpeg_path = configured_ffmpeg_path();
            let frame_cache = RepresentativeFrameCache::default();
            let settings_path = app.path().app_config_dir()?.join("settings.json");
            let system_download_directory = app.path().download_dir().ok();
            let yt_dlp_options = YtDlpOptions::load(
                configured_deno_path(),
                settings_path,
                system_download_directory,
            );
            let runner = RestrictedProcessRunner::new(
                engine_path.clone(),
                INSPECT_TIMEOUT,
                MAX_ENGINE_OUTPUT_BYTES,
            );
            let frame_generator = RepresentativeFrameGenerator::with_options(
                RestrictedProcessRunner::new(
                    engine_path.clone(),
                    FRAME_TIMEOUT,
                    MAX_STREAM_OUTPUT_BYTES,
                ),
                RestrictedProcessRunner::new(
                    ffmpeg_path.clone(),
                    FRAME_TIMEOUT,
                    MAX_FRAME_OUTPUT_BYTES,
                ),
                frame_cache.clone(),
                yt_dlp_options.clone(),
            );

            app.manage(AppState::new(YtDlpEngine::with_options(
                runner,
                frame_cache,
                yt_dlp_options.clone(),
            )));
            app.manage(DownloadCoordinator::new(YtDlpDownloader::with_options(
                engine_path,
                ffmpeg_path,
                yt_dlp_options.clone(),
            )));
            app.manage(yt_dlp_options);
            app.manage(ThumbnailFetcher);
            app.manage(frame_generator);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::download::suggest_download_file_name,
            commands::download::prepare_download_task,
            commands::download::prepare_default_download_task,
            commands::download::start_download,
            commands::download::cancel_download,
            commands::media::inspect_url,
            commands::media::cancel_inspection,
            commands::settings::get_app_settings,
            commands::settings::set_youtube_cookie_mode,
            commands::settings::configure_youtube_cookie_file,
            commands::settings::configure_download_directory,
            commands::thumbnail::fetch_thumbnail,
            commands::thumbnail::generate_representative_frame
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Velo");
}
