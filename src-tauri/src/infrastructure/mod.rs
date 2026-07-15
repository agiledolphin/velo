#[cfg(test)]
mod mock_media_engine;
mod process_runner;
mod representative_frame;
mod thumbnail_fetcher;
mod yt_dlp_downloader;
mod yt_dlp_engine;
mod yt_dlp_options;

pub(crate) use process_runner::RestrictedProcessRunner;
pub(crate) use representative_frame::{
    RepresentativeFrameCache, RepresentativeFrameError, RepresentativeFrameGenerator,
    StreamReference,
};
pub(crate) use thumbnail_fetcher::{ThumbnailError, ThumbnailFetcher};
pub(crate) use yt_dlp_downloader::{YtDlpDownloader, configured_ffmpeg_path};
pub use yt_dlp_engine::{YtDlpEngine, configured_yt_dlp_path};
pub(crate) use yt_dlp_options::{
    SettingsSnapshot, YoutubeCookieMode, YtDlpOptions, configured_deno_path,
};
