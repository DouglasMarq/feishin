use std::fs::{DirBuilder, DirEntry, OpenOptions, Permissions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use super::auth::MusicAuth;
use super::error::MusicError;
use crate::{YouTubeError, YtDlp, unix_now};

const RUNTIME_SUBDIRECTORY: &str = "feishin-rs";
const PRIVATE_DIRECTORY_MODE: u32 = 0o700;
const PRIVATE_FILE_MODE: u32 = 0o600;
const COOKIE_FILE_PREFIX: &str = "youtube-music-cookies-";
const STALE_AFTER: Duration = Duration::from_secs(600);
const NETSCAPE_HEADER: &[u8] = b"# Netscape HTTP Cookie File\n";
const READING_COOKIES: &str = "could not read the exported cookies";
const PREPARING_PRIVATE_FOLDER: &str = "could not prepare a private folder";
const CREATING_COOKIE_FILE: &str = "could not create the cookie file";

static EXPORT_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn private_runtime_dir() -> Option<PathBuf> {
    non_empty_directory("XDG_RUNTIME_DIR")
        .or_else(per_user_temporary_dir)
        .map(|directory| directory.join(RUNTIME_SUBDIRECTORY))
}

fn non_empty_directory(variable_name: &str) -> Option<PathBuf> {
    std::env::var_os(variable_name)
        .filter(|directory| !directory.is_empty())
        .map(PathBuf::from)
}

#[cfg(target_os = "macos")]
fn per_user_temporary_dir() -> Option<PathBuf> {
    non_empty_directory("TMPDIR")
}

#[cfg(not(target_os = "macos"))]
fn per_user_temporary_dir() -> Option<PathBuf> {
    None
}

pub async fn sign_in_with_browser(
    yt_dlp: &YtDlp,
    private_dir: &Path,
) -> Result<MusicAuth, MusicError> {
    let browser = yt_dlp
        .cookie_browser()
        .ok_or_else(|| MusicError::SignIn("no browser is selected".to_owned()))?;
    prepare_private_dir(private_dir)?;
    remove_stale_cookie_files(private_dir);
    let cookie_file = PrivateFile::create(private_dir)?;
    let exported = yt_dlp.export_cookies(cookie_file.path()).await;
    let text = read_exported_cookies(cookie_file.path(), &exported)?;
    drop(cookie_file);
    match (
        MusicAuth::from_netscape_cookies(&text, unix_now()),
        exported,
    ) {
        (Some(auth), _) => Ok(auth),
        (None, Err(error)) => Err(MusicError::CookieExport(error)),
        (None, Ok(())) => Err(MusicError::SignIn(format!(
            "{} has no YouTube session",
            browser.label()
        ))),
    }
}

fn read_exported_cookies(
    path: &Path,
    exported: &Result<(), YouTubeError>,
) -> Result<String, MusicError> {
    match (std::fs::read_to_string(path), exported) {
        (Ok(text), _) => Ok(text),
        (Err(error), Ok(())) => Err(cookie_file_error(READING_COOKIES, error)),
        (Err(error), Err(_)) => {
            tracing::debug!("yt-dlp left no cookie file: {error}");
            Ok(String::new())
        }
    }
}

fn cookie_file_error(context: &'static str, error: std::io::Error) -> MusicError {
    MusicError::CookieFile {
        context,
        source: Arc::new(error),
    }
}

fn prepare_private_dir(directory: &Path) -> Result<(), MusicError> {
    let failed = |error: std::io::Error| cookie_file_error(PREPARING_PRIVATE_FOLDER, error);
    DirBuilder::new()
        .recursive(true)
        .mode(PRIVATE_DIRECTORY_MODE)
        .create(directory)
        .map_err(failed)?;
    let metadata = std::fs::symlink_metadata(directory).map_err(failed)?;
    if !metadata.is_dir() {
        return Err(MusicError::SignIn(
            "the private cookie folder is not a folder".to_owned(),
        ));
    }
    std::fs::set_permissions(directory, Permissions::from_mode(PRIVATE_DIRECTORY_MODE))
        .map_err(failed)
}

fn remove_stale_cookie_files(directory: &Path) {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            tracing::debug!("could not look for stale cookie files: {error}");
            return;
        }
    };
    for entry in entries.flatten().filter(is_stale_cookie_file) {
        if let Err(error) = std::fs::remove_file(entry.path()) {
            tracing::warn!("could not delete a stale cookie file: {error}");
        }
    }
}

fn is_stale_cookie_file(entry: &DirEntry) -> bool {
    let is_cookie_file = entry
        .file_name()
        .to_string_lossy()
        .starts_with(COOKIE_FILE_PREFIX);
    let age = entry
        .metadata()
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok());
    is_cookie_file && age.is_some_and(|age| age > STALE_AFTER)
}

pub(super) struct PrivateFile {
    path: PathBuf,
}

impl PrivateFile {
    pub(super) fn create(directory: &Path) -> Result<Self, MusicError> {
        let number = EXPORT_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = directory.join(format!(
            "{COOKIE_FILE_PREFIX}{}-{number}.txt",
            std::process::id()
        ));
        if let Err(error) = std::fs::remove_file(&path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::debug!("could not remove a stale cookie file: {error}");
        }
        let failed = |error: std::io::Error| cookie_file_error(CREATING_COOKIE_FILE, error);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(PRIVATE_FILE_MODE)
            .open(&path)
            .map_err(failed)?;
        let created = Self { path };
        file.write_all(NETSCAPE_HEADER).map_err(failed)?;
        Ok(created)
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PrivateFile {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_file(&self.path) {
            tracing::warn!("could not delete the exported cookie file: {error}");
        }
    }
}
