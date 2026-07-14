use tauri::State;

use crate::infrastructure::{
    RepresentativeFrameError, RepresentativeFrameGenerator, ThumbnailError, ThumbnailFetcher,
};

#[tauri::command]
pub async fn fetch_thumbnail(
    url: String,
    fetcher: State<'_, ThumbnailFetcher>,
) -> Result<String, ThumbnailError> {
    fetcher.fetch_data_url(&url).await
}

#[tauri::command]
pub async fn generate_representative_frame(
    url: String,
    generator: State<'_, RepresentativeFrameGenerator>,
) -> Result<String, RepresentativeFrameError> {
    generator.generate_data_url(&url).await
}
