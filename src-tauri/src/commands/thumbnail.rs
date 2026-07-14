use tauri::State;

use crate::infrastructure::{ThumbnailError, ThumbnailFetcher};

#[tauri::command]
pub async fn fetch_thumbnail(
    url: String,
    fetcher: State<'_, ThumbnailFetcher>,
) -> Result<String, ThumbnailError> {
    fetcher.fetch_data_url(&url).await
}
