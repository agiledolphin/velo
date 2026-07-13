use tauri::State;

use crate::{
    application::AppState,
    domain::{InspectError, MediaInfo},
};

#[tauri::command]
pub async fn inspect_url(
    request_id: String,
    url: String,
    state: State<'_, AppState>,
) -> Result<MediaInfo, InspectError> {
    state.inspect(&request_id, &url).await
}

#[tauri::command]
pub fn cancel_inspection(request_id: String, state: State<'_, AppState>) -> bool {
    state.cancel(&request_id)
}
