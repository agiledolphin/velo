use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use crate::domain::InspectError;

const MAX_COOKIE_FILE_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Clone)]
pub struct YtDlpOptions {
    deno: PathBuf,
    cookie_file: Arc<RwLock<Option<PathBuf>>>,
}

impl YtDlpOptions {
    pub fn new(deno: impl Into<PathBuf>) -> Self {
        let deno = deno.into();
        assert!(deno.is_absolute(), "Deno path must be absolute");
        Self {
            deno,
            cookie_file: Arc::new(RwLock::new(None)),
        }
    }

    pub fn append_engine_arguments(&self, arguments: &mut Vec<OsString>) {
        arguments.push(OsString::from("--js-runtimes"));
        arguments.push(OsString::from(format!(
            "deno:{}",
            self.deno.to_string_lossy()
        )));
        arguments.push(OsString::from("--no-remote-components"));
        if let Some(path) = self.cookie_file() {
            arguments.push(OsString::from("--cookies"));
            arguments.push(path.into_os_string());
        }
    }

    pub fn configure_cookie_file(&self, path: Option<&str>) -> Result<bool, InspectError> {
        let Some(path) = path else {
            *self
                .cookie_file
                .write()
                .unwrap_or_else(|error| error.into_inner()) = None;
            return Ok(false);
        };
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err(InspectError::invalid_cookie_file());
        }
        let metadata = fs::metadata(&path).map_err(|_| InspectError::invalid_cookie_file())?;
        if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_COOKIE_FILE_BYTES {
            return Err(InspectError::invalid_cookie_file());
        }
        let bytes = fs::read(&path).map_err(|_| InspectError::invalid_cookie_file())?;
        let raw_first_line = bytes
            .split(|byte| *byte == b'\n')
            .next()
            .unwrap_or_default();
        let first_line = raw_first_line.strip_suffix(b"\r").unwrap_or(raw_first_line);
        if first_line != b"# Netscape HTTP Cookie File" && first_line != b"# HTTP Cookie File" {
            return Err(InspectError::invalid_cookie_file());
        }
        let canonical = path
            .canonicalize()
            .map_err(|_| InspectError::invalid_cookie_file())?;
        *self
            .cookie_file
            .write()
            .unwrap_or_else(|error| error.into_inner()) = Some(canonical);
        Ok(true)
    }

    fn cookie_file(&self) -> Option<PathBuf> {
        self.cookie_file
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
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

    #[test]
    fn adds_a_fixed_local_runtime_without_remote_components() {
        let mut arguments = Vec::new();
        options().append_engine_arguments(&mut arguments);
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
    fn accepts_only_bounded_netscape_cookie_files() {
        let options = options();
        let path = env::temp_dir().join(format!("velo-cookies-{}.txt", std::process::id()));
        fs::write(
            &path,
            b"# Netscape HTTP Cookie File\r\n.example\tTRUE\t/\tTRUE\t0\tname\tvalue\r\n",
        )
        .expect("fixture should be written");
        assert!(
            options
                .configure_cookie_file(path.to_str())
                .expect("valid cookie file")
        );
        let mut arguments = Vec::new();
        options.append_engine_arguments(&mut arguments);
        assert!(arguments.iter().any(|argument| argument == "--cookies"));
        assert!(!options.configure_cookie_file(None).expect("cookie clears"));
        fs::write(&path, b"not a cookie file").expect("invalid fixture should be written");
        assert!(options.configure_cookie_file(path.to_str()).is_err());
        fs::remove_file(path).expect("fixture should be removed");
    }
}
