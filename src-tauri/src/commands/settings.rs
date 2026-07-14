use tauri::State;

use crate::{domain::InspectError, infrastructure::YtDlpOptions};

#[tauri::command]
pub fn configure_cookie_file(
    path: Option<String>,
    options: State<'_, YtDlpOptions>,
) -> Result<bool, InspectError> {
    options.configure_cookie_file(path.as_deref())
}
