use std::{
    collections::VecDeque,
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
};

use serde::{Deserialize, Serialize};
use url::Url;

use crate::domain::InspectError;

const MAX_COOKIE_FILE_BYTES: u64 = 5 * 1024 * 1024;
const SETTINGS_SCHEMA_VERSION: u32 = 1;
const MAX_AUTHENTICATED_SOURCES: usize = 32;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum YoutubeCookieMode {
    Disabled,
    #[default]
    OnDemand,
    Always,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
struct AppSettings {
    schema_version: u32,
    sites: SiteSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            sites: SiteSettings::default(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct SiteSettings {
    youtube: YoutubeSettings,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
struct YoutubeSettings {
    cookie_mode: YoutubeCookieMode,
    cookie_file_path: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CookieFileStatus {
    NotConfigured,
    Ready,
    Missing,
    Invalid,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshot {
    schema_version: u32,
    sites: SiteSettingsSnapshot,
}

#[derive(Clone, Debug, Serialize)]
struct SiteSettingsSnapshot {
    youtube: YoutubeSettingsSnapshot,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct YoutubeSettingsSnapshot {
    cookie_mode: YoutubeCookieMode,
    cookie_file_path: Option<String>,
    cookie_file_status: CookieFileStatus,
}

#[derive(Clone)]
pub struct YtDlpOptions {
    deno: PathBuf,
    settings_path: Option<PathBuf>,
    settings: Arc<RwLock<AppSettings>>,
    authenticated_sources: Arc<Mutex<VecDeque<String>>>,
}

impl YtDlpOptions {
    pub fn new(deno: impl Into<PathBuf>) -> Self {
        Self::from_settings(deno.into(), None, AppSettings::default())
    }

    pub fn load(deno: impl Into<PathBuf>, settings_path: PathBuf) -> Self {
        let settings = fs::read(&settings_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<AppSettings>(&bytes).ok())
            .filter(|settings| settings.schema_version == SETTINGS_SCHEMA_VERSION)
            .unwrap_or_default();
        Self::from_settings(deno.into(), Some(settings_path), settings)
    }

    fn from_settings(deno: PathBuf, settings_path: Option<PathBuf>, settings: AppSettings) -> Self {
        assert!(deno.is_absolute(), "Deno path must be absolute");
        Self {
            deno,
            settings_path,
            settings: Arc::new(RwLock::new(settings)),
            authenticated_sources: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn append_engine_arguments(
        &self,
        arguments: &mut Vec<OsString>,
        source: &str,
        use_cookie: bool,
    ) {
        arguments.push(OsString::from("--js-runtimes"));
        arguments.push(OsString::from(format!(
            "deno:{}",
            self.deno.to_string_lossy()
        )));
        arguments.push(OsString::from("--no-remote-components"));
        if use_cookie && let Some(path) = self.youtube_cookie_file(source) {
            arguments.push(OsString::from("--cookies"));
            arguments.push(path.into_os_string());
        }
    }

    pub fn should_use_cookie_initially(&self, source: &str) -> bool {
        self.youtube_cookie_mode(source) == Some(YoutubeCookieMode::Always)
            && self.youtube_cookie_file(source).is_some()
    }

    pub fn should_retry_with_cookie(&self, source: &str) -> bool {
        self.youtube_cookie_mode(source) == Some(YoutubeCookieMode::OnDemand)
            && self.youtube_cookie_file(source).is_some()
    }

    pub fn should_use_cookie_for_media(&self, source: &str) -> bool {
        match self.youtube_cookie_mode(source) {
            Some(YoutubeCookieMode::Always) => self.youtube_cookie_file(source).is_some(),
            Some(YoutubeCookieMode::OnDemand) => {
                self.youtube_cookie_file(source).is_some()
                    && self
                        .authenticated_sources()
                        .iter()
                        .any(|known| known == source)
            }
            Some(YoutubeCookieMode::Disabled) | None => false,
        }
    }

    pub fn remember_authenticated_source(&self, source: &str) {
        if !is_youtube_url(source) {
            return;
        }
        let mut sources = self.authenticated_sources();
        sources.retain(|known| known != source);
        while sources.len() >= MAX_AUTHENTICATED_SOURCES {
            sources.pop_front();
        }
        sources.push_back(source.to_owned());
    }

    pub fn settings_snapshot(&self) -> SettingsSnapshot {
        let settings = self.settings();
        let path = settings.sites.youtube.cookie_file_path.clone();
        SettingsSnapshot {
            schema_version: settings.schema_version,
            sites: SiteSettingsSnapshot {
                youtube: YoutubeSettingsSnapshot {
                    cookie_mode: settings.sites.youtube.cookie_mode,
                    cookie_file_status: cookie_file_status(path.as_deref()),
                    cookie_file_path: path.map(|value| value.to_string_lossy().into_owned()),
                },
            },
        }
    }

    pub fn set_youtube_cookie_mode(
        &self,
        mode: YoutubeCookieMode,
    ) -> Result<SettingsSnapshot, InspectError> {
        self.update_settings(|settings| settings.sites.youtube.cookie_mode = mode)?;
        Ok(self.settings_snapshot())
    }

    pub fn configure_youtube_cookie_file(
        &self,
        path: Option<&str>,
    ) -> Result<SettingsSnapshot, InspectError> {
        let path = match path {
            Some(path) => Some(validated_cookie_file(Path::new(path))?),
            None => None,
        };
        self.update_settings(|settings| settings.sites.youtube.cookie_file_path = path)?;
        Ok(self.settings_snapshot())
    }

    fn update_settings(&self, update: impl FnOnce(&mut AppSettings)) -> Result<(), InspectError> {
        let mut next = self.settings();
        update(&mut next);
        self.persist(&next)?;
        *self
            .settings
            .write()
            .unwrap_or_else(|error| error.into_inner()) = next;
        self.authenticated_sources().clear();
        Ok(())
    }

    fn persist(&self, settings: &AppSettings) -> Result<(), InspectError> {
        let Some(path) = &self.settings_path else {
            return Ok(());
        };
        let parent = path
            .parent()
            .ok_or_else(InspectError::settings_unavailable)?;
        fs::create_dir_all(parent).map_err(|_| InspectError::settings_unavailable())?;
        let bytes = serde_json::to_vec_pretty(settings)
            .map_err(|_| InspectError::settings_unavailable())?;
        fs::write(path, bytes).map_err(|_| InspectError::settings_unavailable())
    }

    fn settings(&self) -> AppSettings {
        self.settings
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    fn youtube_cookie_mode(&self, source: &str) -> Option<YoutubeCookieMode> {
        is_youtube_url(source).then(|| self.settings().sites.youtube.cookie_mode)
    }

    fn youtube_cookie_file(&self, source: &str) -> Option<PathBuf> {
        if !is_youtube_url(source) {
            return None;
        }
        let path = self.settings().sites.youtube.cookie_file_path?;
        validated_cookie_file(&path).ok()
    }

    fn authenticated_sources(&self) -> std::sync::MutexGuard<'_, VecDeque<String>> {
        self.authenticated_sources
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn is_youtube_url(source: &str) -> bool {
    let Ok(url) = Url::parse(source) else {
        return false;
    };
    let Some(host) = url.host_str().map(str::to_ascii_lowercase) else {
        return false;
    };
    host == "youtu.be"
        || host == "youtube.com"
        || host.ends_with(".youtube.com")
        || host == "youtube-nocookie.com"
        || host.ends_with(".youtube-nocookie.com")
}

fn cookie_file_status(path: Option<&Path>) -> CookieFileStatus {
    let Some(path) = path else {
        return CookieFileStatus::NotConfigured;
    };
    if !path.is_file() {
        return CookieFileStatus::Missing;
    }
    if validated_cookie_file(path).is_ok() {
        CookieFileStatus::Ready
    } else {
        CookieFileStatus::Invalid
    }
}

fn validated_cookie_file(path: &Path) -> Result<PathBuf, InspectError> {
    if !path.is_absolute() {
        return Err(InspectError::invalid_cookie_file());
    }
    let metadata = fs::metadata(path).map_err(|_| InspectError::invalid_cookie_file())?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_COOKIE_FILE_BYTES {
        return Err(InspectError::invalid_cookie_file());
    }
    let bytes = fs::read(path).map_err(|_| InspectError::invalid_cookie_file())?;
    let raw_first_line = bytes
        .split(|byte| *byte == b'\n')
        .next()
        .unwrap_or_default();
    let first_line = raw_first_line.strip_suffix(b"\r").unwrap_or(raw_first_line);
    if first_line != b"# Netscape HTTP Cookie File" && first_line != b"# HTTP Cookie File" {
        return Err(InspectError::invalid_cookie_file());
    }
    path.canonicalize()
        .map_err(|_| InspectError::invalid_cookie_file())
}

pub fn configured_deno_path() -> PathBuf {
    if let Some(path) = env::var_os("VELO_DENO_PATH").map(PathBuf::from)
        && path.is_absolute()
    {
        return path;
    }
    let binary_name = if cfg!(windows) { "deno.exe" } else { "deno" };
    if let Ok(current_executable) = env::current_exe()
        && let Some(directory) = current_executable.parent()
    {
        let sibling = directory.join(binary_name);
        if sibling.is_file() {
            return sibling;
        }
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("binaries")
        .join(local_deno_sidecar_name())
}

fn local_deno_sidecar_name() -> &'static str {
    match (env::consts::OS, env::consts::ARCH) {
        ("macos", "aarch64") => "deno-aarch64-apple-darwin",
        ("macos", "x86_64") => "deno-x86_64-apple-darwin",
        ("linux", "aarch64") => "deno-aarch64-unknown-linux-gnu",
        ("linux", "x86_64") => "deno-x86_64-unknown-linux-gnu",
        ("windows", "x86_64") => "deno-x86_64-pc-windows-msvc.exe",
        _ => {
            if cfg!(windows) {
                "deno.exe"
            } else {
                "deno"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> YtDlpOptions {
        YtDlpOptions::new(if cfg!(windows) {
            PathBuf::from(r"C:\trusted\deno.exe")
        } else {
            PathBuf::from("/trusted/deno")
        })
    }

    fn cookie_fixture(label: &str) -> PathBuf {
        let path = env::temp_dir().join(format!(
            "velo-cookies-{label}-{}-{}.txt",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be valid")
                .as_nanos()
        ));
        fs::write(
            &path,
            b"# Netscape HTTP Cookie File\r\n.youtube.com\tTRUE\t/\tTRUE\t0\tname\tvalue\r\n",
        )
        .expect("fixture should be written");
        path
    }

    #[test]
    fn adds_a_fixed_local_runtime_without_remote_components() {
        let mut arguments = Vec::new();
        options().append_engine_arguments(&mut arguments, "https://video.example/watch", false);
        let strings = arguments
            .iter()
            .map(|value| value.to_string_lossy())
            .collect::<Vec<_>>();
        assert_eq!(strings[0], "--js-runtimes");
        assert!(strings[1].starts_with("deno:"));
        assert_eq!(strings[2], "--no-remote-components");
        assert!(!strings.iter().any(|value| value == "--remote-components"));
    }

    #[test]
    fn scopes_configured_cookies_to_youtube_and_on_demand_retry() {
        let options = options();
        let path = cookie_fixture("scope");
        options
            .configure_youtube_cookie_file(path.to_str())
            .expect("valid cookie file");

        assert!(!options.should_use_cookie_initially("https://youtube.com/watch?v=1"));
        assert!(options.should_retry_with_cookie("https://youtu.be/1"));
        assert!(!options.should_retry_with_cookie("https://x.com/video"));

        let mut youtube_arguments = Vec::new();
        options.append_engine_arguments(
            &mut youtube_arguments,
            "https://youtube.com/watch?v=1",
            true,
        );
        assert!(
            youtube_arguments
                .iter()
                .any(|argument| argument == "--cookies")
        );

        let mut other_arguments = Vec::new();
        options.append_engine_arguments(&mut other_arguments, "https://x.com/video", true);
        assert!(
            !other_arguments
                .iter()
                .any(|argument| argument == "--cookies")
        );
        fs::remove_file(path).expect("fixture should be removed");
    }

    #[test]
    fn persists_versioned_youtube_settings() {
        let root = env::temp_dir().join(format!(
            "velo-settings-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be valid")
                .as_nanos()
        ));
        let settings_path = root.join("settings.json");
        let cookie_path = cookie_fixture("persist");
        let options = YtDlpOptions::load(configured_deno_path(), settings_path.clone());
        options
            .configure_youtube_cookie_file(cookie_path.to_str())
            .expect("cookie should persist");
        options
            .set_youtube_cookie_mode(YoutubeCookieMode::Always)
            .expect("mode should persist");

        let reloaded = YtDlpOptions::load(configured_deno_path(), settings_path);
        assert!(reloaded.should_use_cookie_initially("https://www.youtube.com/watch?v=1"));
        assert_eq!(reloaded.settings_snapshot().schema_version, 1);

        fs::remove_dir_all(root).expect("settings directory should be removed");
        fs::remove_file(cookie_path).expect("cookie fixture should be removed");
    }
}
