//! Local persistence: settings, credentials, queue restore, Feishin import and the cover art cache.

use std::path::PathBuf;

pub mod cover_cache;
pub mod credentials;
pub mod feishin_import;
pub mod queue_store;
pub mod settings;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid data: {0}")]
    Invalid(String),
    #[error("keyring error: {0}")]
    Keyring(String),
    #[error("network error: {0}")]
    Network(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;

/// XDG directories for the app (`feishin-rs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub data_dir: PathBuf,
}

impl Paths {
    /// `~/.config/feishin-rs`, `~/.cache/feishin-rs`, `~/.local/share/feishin-rs` (respecting XDG env vars).
    pub fn from_env() -> Result<Self> {
        todo!()
    }
}
