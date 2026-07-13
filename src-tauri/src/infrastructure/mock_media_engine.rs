use url::Url;

use crate::{
    application::MediaEngine,
    domain::{InspectError, MediaFormat, MediaInfo},
};

pub struct MockMediaEngine;

impl MediaEngine for MockMediaEngine {
    fn inspect(&self, source: &str) -> Result<MediaInfo, InspectError> {
        let url = Url::parse(source).map_err(|_| InspectError::invalid_url())?;

        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err(InspectError::invalid_url());
        }

        let site = url.host_str().unwrap_or("Unknown site").to_string();

        Ok(MediaInfo {
            source_url: url.to_string(),
            title: "一段等待被留住的流光".into(),
            site,
            thumbnail_url: None,
            duration_seconds: Some(213),
            formats: vec![
                MediaFormat {
                    id: "mock-1080p".into(),
                    label: "1080p · 推荐".into(),
                    container: "mp4".into(),
                    width: Some(1920),
                    height: Some(1080),
                    filesize_bytes: Some(86 * 1024 * 1024),
                    has_video: true,
                    has_audio: true,
                },
                MediaFormat {
                    id: "mock-720p".into(),
                    label: "720p · 轻量".into(),
                    container: "mp4".into(),
                    width: Some(1280),
                    height: Some(720),
                    filesize_bytes: Some(48 * 1024 * 1024),
                    has_video: true,
                    has_audio: true,
                },
            ],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_mock_media_for_web_url() {
        let media = MockMediaEngine
            .inspect("https://video.example/watch?id=42")
            .expect("valid URLs should be inspected");

        assert_eq!(media.site, "video.example");
        assert_eq!(media.formats.len(), 2);
        assert_eq!(media.formats[0].height, Some(1080));
    }

    #[test]
    fn rejects_non_web_urls() {
        let error = MockMediaEngine
            .inspect("file:///tmp/video.mp4")
            .expect_err("file URLs must be rejected");

        assert_eq!(error.code, "invalid_url");
    }
}
