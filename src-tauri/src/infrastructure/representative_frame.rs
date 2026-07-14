use std::{collections::BTreeMap, ffi::OsString, sync::Arc};

use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use url::Url;

use super::process_runner::{ProcessError, ProcessRunner};

const FRAME_FORMAT: &str = "bestvideo[height<=480]/best[height<=480]/worstvideo/worst";
const FRAME_FILTER: &str = "scale=640:-2:force_original_aspect_ratio=decrease";
const STREAM_TEMPLATE: &str = r#"{"url":%(url)j,"httpHeaders":%(http_headers)j}"#;
const MAX_HEADER_VALUE_BYTES: usize = 4 * 1024;
const MAX_HEADERS_BYTES: usize = 16 * 1024;

pub struct RepresentativeFrameGenerator {
    yt_dlp: Arc<dyn ProcessRunner>,
    ffmpeg: Arc<dyn ProcessRunner>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RepresentativeFrameError {
    pub code: &'static str,
    pub message: &'static str,
}

impl RepresentativeFrameError {
    fn unavailable() -> Self {
        Self {
            code: "representative_frame_unavailable",
            message: "暂时无法生成视频代表帧。",
        }
    }
}

impl RepresentativeFrameGenerator {
    pub fn new(yt_dlp: impl ProcessRunner, ffmpeg: impl ProcessRunner) -> Self {
        Self {
            yt_dlp: Arc::new(yt_dlp),
            ffmpeg: Arc::new(ffmpeg),
        }
    }

    pub async fn generate_data_url(
        &self,
        source: &str,
    ) -> Result<String, RepresentativeFrameError> {
        let source = validated_web_url(source)?;
        let stream_output = self
            .yt_dlp
            .run(&stream_arguments(source.as_str()))
            .await
            .map_err(map_process_error)?;
        if !stream_output.success {
            return Err(RepresentativeFrameError::unavailable());
        }
        let stream = parse_stream_reference(&stream_output.stdout)?;
        let frame_output = self
            .ffmpeg
            .run(&frame_arguments(&stream))
            .await
            .map_err(map_process_error)?;
        if !frame_output.success || !is_jpeg(&frame_output.stdout) {
            return Err(RepresentativeFrameError::unavailable());
        }

        Ok(format!(
            "data:image/jpeg;base64,{}",
            STANDARD.encode(frame_output.stdout)
        ))
    }
}

fn stream_arguments(source: &str) -> Vec<OsString> {
    [
        "--ignore-config",
        "--no-plugin-dirs",
        "--no-js-runtimes",
        "--no-remote-components",
        "--no-exec",
        "--no-cache-dir",
        "--no-update",
        "--no-playlist",
        "--no-warnings",
        "--simulate",
        "--format",
        FRAME_FORMAT,
        "--print",
        STREAM_TEMPLATE,
        "--",
        source,
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}

fn frame_arguments(stream: &StreamReference) -> Vec<OsString> {
    let mut arguments = [
        "-hide_banner",
        "-loglevel",
        "error",
        "-nostdin",
        "-protocol_whitelist",
        "crypto,http,https,tcp,tls,httpproxy",
    ]
    .into_iter()
    .map(OsString::from)
    .collect::<Vec<_>>();

    if let Some(headers) = safe_ffmpeg_headers(&stream.http_headers) {
        arguments.push(OsString::from("-headers"));
        arguments.push(OsString::from(headers));
    }

    for value in [
        "-i",
        stream.url.as_str(),
        "-ss",
        "1",
        "-map",
        "0:v:0",
        "-an",
        "-frames:v",
        "1",
        "-vf",
        FRAME_FILTER,
        "-f",
        "image2pipe",
        "-c:v",
        "mjpeg",
        "-q:v",
        "4",
        "pipe:1",
    ] {
        arguments.push(OsString::from(value));
    }
    arguments
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawStreamReference {
    url: String,
    #[serde(default)]
    http_headers: BTreeMap<String, String>,
}

struct StreamReference {
    url: Url,
    http_headers: BTreeMap<String, String>,
}

fn parse_stream_reference(bytes: &[u8]) -> Result<StreamReference, RepresentativeFrameError> {
    let raw: RawStreamReference =
        serde_json::from_slice(bytes).map_err(|_| RepresentativeFrameError::unavailable())?;
    Ok(StreamReference {
        url: validated_web_url(&raw.url)?,
        http_headers: raw.http_headers,
    })
}

fn validated_web_url(source: &str) -> Result<Url, RepresentativeFrameError> {
    let url = Url::parse(source).map_err(|_| RepresentativeFrameError::unavailable())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(RepresentativeFrameError::unavailable());
    }
    Ok(url)
}

fn safe_ffmpeg_headers(headers: &BTreeMap<String, String>) -> Option<String> {
    let mut output = String::new();
    for (name, value) in headers {
        if !matches!(
            name.to_ascii_lowercase().as_str(),
            "accept" | "accept-language" | "origin" | "referer" | "user-agent"
        ) || value.is_empty()
            || value.len() > MAX_HEADER_VALUE_BYTES
            || value.contains(['\r', '\n'])
        {
            continue;
        }
        let line = format!("{name}: {value}\r\n");
        if output.len().saturating_add(line.len()) > MAX_HEADERS_BYTES {
            break;
        }
        output.push_str(&line);
    }
    (!output.is_empty()).then_some(output)
}

fn is_jpeg(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes.starts_with(&[0xff, 0xd8, 0xff]) && bytes.ends_with(&[0xff, 0xd9])
}

fn map_process_error(_error: ProcessError) -> RepresentativeFrameError {
    RepresentativeFrameError::unavailable()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_hardened_stream_and_frame_arguments() {
        let stream_arguments = stream_arguments("https://video.example/watch?v=1&x=true")
            .into_iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(stream_arguments.iter().any(|value| value == "--no-exec"));
        assert!(stream_arguments.iter().any(|value| value == FRAME_FORMAT));
        assert_eq!(stream_arguments[stream_arguments.len() - 2], "--");

        let stream = parse_stream_reference(
            br#"{"url":"https://cdn.example/video.m3u8","httpHeaders":{"User-Agent":"Velo","Cookie":"secret","Referer":"https://video.example/"}}"#,
        )
        .expect("stream reference should parse");
        let frame_arguments = frame_arguments(&stream)
            .into_iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let headers = frame_arguments
            .iter()
            .find(|value| value.contains("User-Agent"))
            .expect("safe headers should be forwarded");
        assert!(headers.contains("Referer"));
        assert!(!headers.contains("Cookie"));
        assert!(frame_arguments.iter().any(|value| value == "1"));
        assert!(frame_arguments.iter().any(|value| value == "pipe:1"));
    }

    #[test]
    fn rejects_unsafe_streams_headers_and_non_jpeg_output() {
        assert!(parse_stream_reference(br#"{"url":"file:///tmp/video.mp4"}"#).is_err());
        let headers = BTreeMap::from([
            ("User-Agent".into(), "safe".into()),
            (
                "Referer".into(),
                "https://example.test/\r\nCookie: bad".into(),
            ),
        ]);
        assert_eq!(
            safe_ffmpeg_headers(&headers),
            Some("User-Agent: safe\r\n".into())
        );
        assert!(is_jpeg(&[0xff, 0xd8, 0xff, 0xd9]));
        assert!(!is_jpeg(b"not-an-image"));
    }

    #[tokio::test]
    #[ignore = "requires an explicit public video URL and verified yt-dlp/FFmpeg sidecars"]
    async fn generates_a_configured_real_frame() {
        let url = std::env::var("VELO_INTEGRATION_FRAME_URL")
            .expect("VELO_INTEGRATION_FRAME_URL must be set explicitly");
        let generator = RepresentativeFrameGenerator::new(
            crate::infrastructure::RestrictedProcessRunner::new(
                crate::infrastructure::configured_yt_dlp_path(),
                std::time::Duration::from_secs(45),
                256 * 1024,
            ),
            crate::infrastructure::RestrictedProcessRunner::new(
                crate::infrastructure::configured_ffmpeg_path(),
                std::time::Duration::from_secs(45),
                5 * 1024 * 1024,
            ),
        );

        let frame = generator
            .generate_data_url(&url)
            .await
            .expect("representative frame should be generated");
        assert!(frame.starts_with("data:image/jpeg;base64,"));
        assert!(frame.len() > 1024);

        if let Ok(output) = std::env::var("VELO_INTEGRATION_FRAME_OUTPUT") {
            let encoded = frame
                .strip_prefix("data:image/jpeg;base64,")
                .expect("frame should have a JPEG data URL prefix");
            let bytes = STANDARD
                .decode(encoded)
                .expect("generated frame should contain valid base64");
            std::fs::write(output, bytes).expect("configured frame output should be writable");
        }
    }
}
