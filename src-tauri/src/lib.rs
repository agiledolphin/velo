mod application;
mod commands;
mod domain;
mod infrastructure;

use application::AppState;
use infrastructure::MockMediaEngine;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new(MockMediaEngine))
        .invoke_handler(tauri::generate_handler![commands::media::inspect_url])
        .run(tauri::generate_context!())
        .expect("failed to run Velo");
}
