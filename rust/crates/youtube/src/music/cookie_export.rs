use std::fs::{DirBuilder, OpenOptions, Permissions};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::auth::MusicAuth;
use super::error::MusicError;
use crate::{YtDlp, unix_now};

const RUNTIME_SUBDIRECTORY: &str = "feishin-rs";
const PRIVATE_DIRECTORY_MODE: u32 = 0o700;
const PRIVATE_FILE_MODE: u32 = 0o600;

static EXPORT_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn private_runtime_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|directory| !directory.is_empty())
        .map(|directory| PathBuf::from(directory).join(RUNTIME_SUBDIRECTORY))
}

pub async fn sign_in_with_browser(
    yt_dlp: &YtDlp,
    private_dir: &Path,
) -> Result<MusicAuth, MusicError> {
    let browser = yt_dlp
        .cookie_browser()
        .ok_or_else(|| MusicError::SignIn("no browser is selected".to_owned()))?;
    prepare_private_dir(private_dir)?;
    let cookie_file = PrivateFile::create(private_dir)?;
    let exported = yt_dlp.export_cookies(cookie_file.path()).await;
    let text = std::fs::read_to_string(cookie_file.path()).unwrap_or_default();
    drop(cookie_file);
    match (
        MusicAuth::from_netscape_cookies(&text, unix_now()),
        exported,
    ) {
        (Some(auth), _) => Ok(auth),
        (None, Err(error)) => Err(MusicError::SignIn(error.to_string())),
        (None, Ok(())) => Err(MusicError::SignIn(format!(
            "{} has no YouTube session",
            browser.label()
        ))),
    }
}

fn prepare_private_dir(directory: &Path) -> Result<(), MusicError> {
    let failed = |error: std::io::Error| {
        MusicError::SignIn(format!("could not prepare a private folder: {error}"))
    };
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

struct PrivateFile {
    path: PathBuf,
}

impl PrivateFile {
    fn create(directory: &Path) -> Result<Self, MusicError> {
        let number = EXPORT_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = directory.join(format!(
            "youtube-music-cookies-{}-{number}.txt",
            std::process::id()
        ));
        if let Err(error) = std::fs::remove_file(&path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::debug!("could not remove a stale cookie file: {error}");
        }
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(PRIVATE_FILE_MODE)
            .open(&path)
            .map_err(|error| {
                MusicError::SignIn(format!("could not create the cookie file: {error}"))
            })?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
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
