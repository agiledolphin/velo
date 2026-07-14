mod application;
mod commands;
pub mod domain;
mod infrastructure;

use std::time::Duration;

use application::AppState;
use infrastructure::{RestrictedProcessRunner, YtDlpEngine, configured_yt_dlp_path};

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
        .manage(AppState::new(YtDlpEngine::new(runner)))
        .invoke_handler(tauri::generate_handler![
            commands::media::inspect_url,
            commands::media::cancel_inspection
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Velo");
}
