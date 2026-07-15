use tauri::State;

use crate::{
    domain::InspectError,
    infrastructure::{SettingsSnapshot, YoutubeCookieMode, YtDlpOptions},
};

#[tauri::command]
pub fn get_app_settings(options: State<'_, YtDlpOptions>) -> SettingsSnapshot {
    options.settings_snapshot()
}

#[tauri::command]
pub fn set_youtube_cookie_mode(
    mode: YoutubeCookieMode,
    options: State<'_, YtDlpOptions>,
) -> Result<SettingsSnapshot, InspectError> {
    options.set_youtube_cookie_mode(mode)
}

#[tauri::command]
pub fn configure_youtube_cookie_file(
    path: Option<String>,
    options: State<'_, YtDlpOptions>,
) -> Result<SettingsSnapshot, InspectError> {
    options.configure_youtube_cookie_file(path.as_deref())
}
