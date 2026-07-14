mod application;
mod commands;
pub mod domain;
mod infrastructure;

use std::time::Duration;

use application::AppState;
use infrastructure::{
    RestrictedProcessRunner, ThumbnailFetcher, YtDlpEngine, configured_yt_dlp_path,
};

const INSPECT_TIMEOUT: Duration = Duration::from_secs(45);
const MAX_ENGINE_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let runner = RestrictedProcessRunner::new(
        configured_yt_dlp_path(),
        INSPECT_TIMEOUT,
        MAX_ENGINE_OUTPUT_BYTES,
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new(YtDlpEngine::new(runner)))
        .manage(ThumbnailFetcher)
        .invoke_handler(tauri::generate_handler![
            commands::download::suggest_download_file_name,
            commands::download::prepare_download_task,
            commands::media::inspect_url,
            commands::media::cancel_inspection,
            commands::thumbnail::fetch_thumbnail
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Velo");
}
