use tauri::State;

use crate::{
    application::AppState,
    domain::{InspectError, MediaInfo},
};

#[tauri::command]
pub fn inspect_url(url: String, state: State<'_, AppState>) -> Result<MediaInfo, InspectError> {
    state.inspect(&url)
}
