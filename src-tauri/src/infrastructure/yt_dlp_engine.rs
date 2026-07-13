use std::{
    cmp::Ordering,
    env,
    ffi::OsString,
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::Deserialize;
use url::Url;

use crate::{
    application::{InspectFuture, MediaEngine},
    domain::{InspectError, MediaFormat, MediaInfo},
};

use super::process_runner::{ProcessError, ProcessRunner};

const MAX_FORMATS: usize = 24;

pub struct YtDlpEngine {
    runner: Arc<dyn ProcessRunner>,
}

impl YtDlpEngine {
    pub fn new(runner: impl ProcessRunner) -> Self {
        Self {
            runner: Arc::new(runner),
        }
    }
}

impl MediaEngine for YtDlpEngine {
    fn inspect<'a>(&'a self, source: &'a str) -> InspectFuture<'a> {
        Box::pin(async move {
            let source_url = validate_source_url(source)?;
            let arguments = inspection_arguments(source_url.as_str());
            let output = self
                .runner
                .run(&arguments)
                .await
                .map_err(map_process_error)?;

            if !output.success {
                return Err(classify_engine_failure(&output.stderr));
            }

            parse_media_info(&output.stdout, &source_url)
        })
    }
}

pub fn configured_yt_dlp_path() -> PathBuf {
    if let Some(path) = env::var_os("VELO_YT_DLP_PATH").map(PathBuf::from)
        && path.is_absolute()
    {
        return path;
    }

    let binary_name = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };

    if let Ok(current_executable) = env::current_exe()
        && let Some(directory) = current_executable.parent()
    {
        let sibling = directory.join(binary_name);
        if sibling.is_file() {
            return sibling;
        }
    }

    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../binaries")
        .join(binary_name)
}

fn validate_source_url(source: &str) -> Result<Url, InspectError> {
    let url = Url::parse(source).map_err(|_| InspectError::invalid_url())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(InspectError::invalid_url());
    }
    Ok(url)
}

fn inspection_arguments(source: &str) -> Vec<OsString> {
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
        "--dump-single-json",
        "--",
        source,
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}

fn map_process_error(error: ProcessError) -> InspectError {
    match error {
        ProcessError::SpawnFailed => InspectError::engine_unavailable(),
        ProcessError::TimedOut => InspectError::timed_out(),
        ProcessError::OutputTooLarge => InspectError::output_too_large(),
        ProcessError::WaitFailed | ProcessError::ReadFailed => InspectError::engine_failed(),
    }
}

fn classify_engine_failure(stderr: &[u8]) -> InspectError {
    let message = String::from_utf8_lossy(stderr).to_ascii_lowercase();

    if contains_any(
        &message,
        &[
            "http error 429",
            "too many requests",
            "rate limit",
            "rate-limit",
            "confirm you're not a bot",
            "confirm you’re not a bot",
            "content isn't available, try again later",
        ],
    ) {
        InspectError::rate_limited()
    } else if contains_any(
        &message,
        &[
            "geo-restricted",
            "geo restricted",
            "geographic restriction",
            "not available in your country",
            "not available in your region",
            "available in your country",
            "available in your region",
            "blocked in your country",
        ],
    ) {
        InspectError::geo_restricted()
    } else if contains_any(
        &message,
        &[
            "login required",
            "log in to",
            "sign in to",
            "authentication required",
            "account required",
            "registered users",
            "members-only",
            "members only",
            "age-restricted",
            "age restricted",
            "use --cookies",
            "private video",
        ],
    ) {
        InspectError::authentication_required()
    } else if contains_any(&message, &["unsupported url", "no suitable extractor"]) {
        InspectError::site_unsupported()
    } else if contains_any(
        &message,
        &[
            "video unavailable",
            "video is unavailable",
            "video is not available",
            "content is not available",
            "has been removed",
            "does not exist",
            "video was deleted",
            "content was deleted",
        ],
    ) {
        InspectError::content_unavailable()
    } else if contains_any(
        &message,
        &[
            "http error 401",
            "http error 403",
            "access denied",
            "forbidden",
        ],
    ) {
        InspectError::access_denied()
    } else if contains_any(
        &message,
        &[
            "unable to download webpage",
            "failed to resolve",
            "name or service not known",
            "temporary failure in name resolution",
            "connection refused",
            "connection reset",
            "network is unreachable",
        ],
    ) {
        InspectError::network_failed()
    } else {
        InspectError::engine_failed()
    }
}

fn contains_any(message: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|pattern| message.contains(pattern))
}

fn parse_media_info(bytes: &[u8], source_url: &Url) -> Result<MediaInfo, InspectError> {
    let raw: RawMediaInfo =
        serde_json::from_slice(bytes).map_err(|_| InspectError::invalid_response())?;
    let title =
        normalized_text(raw.title.as_deref(), 200).ok_or_else(InspectError::invalid_response)?;
    let site = normalized_text(raw.webpage_url_domain.as_deref(), 100)
        .or_else(|| source_url.host_str().map(ToOwned::to_owned))
        .ok_or_else(InspectError::invalid_response)?;
    let thumbnail_url = raw.thumbnail.as_deref().and_then(normalized_web_url);
    let mut formats: Vec<_> = raw
        .formats
        .into_iter()
        .filter_map(normalize_format)
        .collect();
    formats.sort_by(compare_formats);
    formats.truncate(MAX_FORMATS);

    if formats.is_empty() {
        return Err(InspectError::no_formats());
    }

    Ok(MediaInfo {
        source_url: source_url.to_string(),
        title,
        site,
        thumbnail_url,
        duration_seconds: raw.duration.and_then(non_negative_u64),
        formats,
    })
}

fn normalize_format(raw: RawMediaFormat) -> Option<MediaFormat> {
    let id = normalized_text(raw.format_id.as_deref(), 100)?;
    let container = normalized_container(raw.ext.as_deref())?;
    let has_video = codec_is_present(raw.vcodec.as_deref());
    let has_audio = codec_is_present(raw.acodec.as_deref());

    if !has_video && !has_audio {
        return None;
    }

    let label = match (has_video, has_audio, raw.height) {
        (true, true, Some(height)) => format!("{height}p · 音视频"),
        (true, false, Some(height)) => format!("{height}p · 仅视频"),
        (true, true, None) => "视频 · 音视频".into(),
        (true, false, None) => "视频 · 仅视频".into(),
        (false, true, _) => "音频".into(),
        (false, false, _) => return None,
    };

    Some(MediaFormat {
        id,
        label,
        container,
        width: raw.width,
        height: raw.height,
        filesize_bytes: raw
            .filesize
            .or(raw.filesize_approx)
            .and_then(non_negative_u64),
        has_video,
        has_audio,
    })
}

fn compare_formats(left: &MediaFormat, right: &MediaFormat) -> Ordering {
    stream_rank(left)
        .cmp(&stream_rank(right))
        .then_with(|| right.height.unwrap_or(0).cmp(&left.height.unwrap_or(0)))
        .then_with(|| right.width.unwrap_or(0).cmp(&left.width.unwrap_or(0)))
        .then_with(|| left.container.cmp(&right.container))
        .then_with(|| left.id.cmp(&right.id))
}

fn stream_rank(format: &MediaFormat) -> u8 {
    match (format.has_video, format.has_audio) {
        (true, true) => 0,
        (true, false) => 1,
        (false, true) => 2,
        (false, false) => 3,
    }
}

fn codec_is_present(codec: Option<&str>) -> bool {
    codec.is_some_and(|value| {
        let value = value.trim();
        !value.is_empty() && !value.eq_ignore_ascii_case("none")
    })
}

fn normalized_container(value: Option<&str>) -> Option<String> {
    let value = value?.trim().to_ascii_lowercase();
    if value.is_empty()
        || value.len() > 12
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
    {
        return None;
    }
    Some(value)
}

fn normalized_text(value: Option<&str>, max_chars: usize) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.chars().take(max_chars).collect())
}

fn normalized_web_url(value: &str) -> Option<String> {
    let url = Url::parse(value).ok()?;
    if matches!(url.scheme(), "http" | "https") && url.host_str().is_some() {
        Some(url.to_string())
    } else {
        None
    }
}

fn non_negative_u64(value: f64) -> Option<u64> {
    if value.is_finite() && value >= 0.0 && value <= u64::MAX as f64 {
        Some(value.round() as u64)
    } else {
        None
    }
}

#[derive(Debug, Deserialize)]
struct RawMediaInfo {
    title: Option<String>,
    webpage_url_domain: Option<String>,
    thumbnail: Option<String>,
    duration: Option<f64>,
    #[serde(default)]
    formats: Vec<RawMediaFormat>,
}

#[derive(Debug, Deserialize)]
struct RawMediaFormat {
    format_id: Option<String>,
    ext: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    filesize: Option<f64>,
    filesize_approx: Option<f64>,
    vcodec: Option<String>,
    acodec: Option<String>,
}

#[cfg(test)]
mod tests {
    use std::{future::ready, sync::Mutex, time::Duration};

    use super::*;
    use crate::infrastructure::process_runner::{ProcessFuture, ProcessOutput};

    const FIXTURE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/yt_dlp_single_video.json"
    ));

    #[derive(Clone)]
    struct StubRunner {
        result: Result<ProcessOutput, ProcessError>,
        arguments: Arc<Mutex<Vec<OsString>>>,
    }

    impl StubRunner {
        fn success(bytes: &[u8]) -> Self {
            Self {
                result: Ok(ProcessOutput {
                    success: true,
                    exit_code: Some(0),
                    stdout: bytes.to_vec(),
                    stderr: Vec::new(),
                }),
                arguments: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn failure(stderr: &[u8]) -> Self {
            Self {
                result: Ok(ProcessOutput {
                    success: false,
                    exit_code: Some(1),
                    stdout: Vec::new(),
                    stderr: stderr.to_vec(),
                }),
                arguments: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl ProcessRunner for StubRunner {
        fn run<'a>(&'a self, arguments: &'a [OsString]) -> ProcessFuture<'a> {
            self.arguments
                .lock()
                .expect("arguments lock should remain available")
                .clone_from(&arguments.to_vec());
            Box::pin(ready(self.result.clone()))
        }
    }

    #[tokio::test]
    async fn maps_fixture_into_domain_media() {
        let engine = YtDlpEngine::new(StubRunner::success(FIXTURE));

        let media = engine
            .inspect("https://video.example/watch?v=42")
            .await
            .expect("fixture should parse");

        assert_eq!(media.title, "一段真实的流光");
        assert_eq!(media.site, "video.example");
        assert_eq!(media.duration_seconds, Some(213));
        assert_eq!(media.formats.len(), 3);
        assert_eq!(media.formats[0].id, "22");
        assert_eq!(media.formats[0].label, "720p · 音视频");
        assert_eq!(media.formats[1].id, "137");
        assert_eq!(media.formats[1].filesize_bytes, Some(86 * 1024 * 1024));
        assert_eq!(media.formats[2].label, "音频");
    }

    #[tokio::test]
    async fn uses_a_fixed_non_downloading_argument_set() {
        let runner = StubRunner::success(FIXTURE);
        let captured = Arc::clone(&runner.arguments);
        let engine = YtDlpEngine::new(runner);

        engine
            .inspect("https://video.example/watch?v=42&list=unsafe")
            .await
            .expect("fixture should parse");

        let arguments = captured.lock().expect("arguments should be captured");
        let strings: Vec<_> = arguments
            .iter()
            .map(|value| value.to_string_lossy())
            .collect();
        assert!(strings.iter().any(|value| value == "--simulate"));
        assert!(strings.iter().any(|value| value == "--ignore-config"));
        assert!(strings.iter().any(|value| value == "--no-plugin-dirs"));
        assert!(strings.iter().any(|value| value == "--no-js-runtimes"));
        assert_eq!(strings[strings.len() - 2], "--");
        assert_eq!(
            strings.last().expect("URL should be the final argument"),
            "https://video.example/watch?v=42&list=unsafe"
        );
    }

    #[tokio::test]
    async fn maps_missing_executable_without_exposing_system_details() {
        let runner = StubRunner {
            result: Err(ProcessError::SpawnFailed),
            arguments: Arc::new(Mutex::new(Vec::new())),
        };
        let engine = YtDlpEngine::new(runner);

        let error = engine
            .inspect("https://video.example/watch?v=42")
            .await
            .expect_err("missing executable should fail");

        assert_eq!(error.code, "engine_unavailable");
        assert!(!error.message.contains('/'));
    }

    #[tokio::test]
    async fn rejects_invalid_json() {
        let engine = YtDlpEngine::new(StubRunner::success(b"not-json"));

        let error = engine
            .inspect("https://video.example/watch?v=42")
            .await
            .expect_err("invalid JSON should fail");

        assert_eq!(error.code, "invalid_engine_response");
    }

    #[tokio::test]
    async fn rejects_media_without_downloadable_formats() {
        let engine = YtDlpEngine::new(StubRunner::success(br#"{"title":"Empty","formats":[]}"#));

        let error = engine
            .inspect("https://video.example/watch?v=42")
            .await
            .expect_err("empty formats should fail");

        assert_eq!(error.code, "no_formats");
    }

    #[test]
    fn classifies_expected_site_failures() {
        let cases = [
            (
                "ERROR: Unsupported URL: https://unknown.example/video",
                "site_unsupported",
            ),
            (
                "ERROR: This video is only available for registered users. Use --cookies",
                "authentication_required",
            ),
            (
                "ERROR: The uploader has not made this video available in your country",
                "geo_restricted",
            ),
            ("ERROR: Sign in to confirm you're not a bot", "rate_limited"),
            ("ERROR: This video is not available", "content_unavailable"),
            (
                "ERROR: Unable to download webpage: HTTP Error 403: Forbidden",
                "access_denied",
            ),
            (
                "ERROR: Unable to download webpage: connection refused",
                "network_failed",
            ),
        ];

        for (stderr, expected_code) in cases {
            assert_eq!(
                classify_engine_failure(stderr.as_bytes()).code,
                expected_code,
                "unexpected classification for {stderr}"
            );
        }
    }

    #[test]
    fn engine_failure_does_not_expose_raw_stderr() {
        let error = classify_engine_failure(
            b"ERROR: extractor crashed while reading /Users/private/secret-cookie.txt",
        );

        assert_eq!(error.code, "engine_failed");
        assert!(!error.message.contains("secret-cookie"));
        assert!(!error.message.contains("/Users/"));
    }

    #[tokio::test]
    async fn maps_nonzero_engine_output_to_a_site_error() {
        let engine = YtDlpEngine::new(StubRunner::failure(
            b"ERROR: This video is only available for registered users. Use --cookies",
        ));

        let error = engine
            .inspect("https://video.example/private")
            .await
            .expect_err("nonzero engine output should fail");

        assert_eq!(error.code, "authentication_required");
    }

    #[tokio::test]
    #[ignore = "requires an explicit URL, a verified yt-dlp binary, and network access"]
    async fn inspects_configured_real_url() {
        let source = env::var("VELO_INTEGRATION_TEST_URL")
            .expect("set VELO_INTEGRATION_TEST_URL to an authorized public video URL");
        let executable = env::var_os("VELO_YT_DLP_PATH")
            .map(PathBuf::from)
            .expect("set VELO_YT_DLP_PATH to the verified absolute binary path");
        let runner = super::super::process_runner::RestrictedProcessRunner::new(
            executable,
            Duration::from_secs(60),
            8 * 1024 * 1024,
        );
        let engine = YtDlpEngine::new(runner);

        let media = engine
            .inspect(&source)
            .await
            .expect("configured integration URL should be inspectable");

        assert!(!media.title.is_empty());
        assert!(!media.formats.is_empty());
        println!(
            "解析成功：{}（{} 个格式）",
            media.title,
            media.formats.len()
        );
    }
}
