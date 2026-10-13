//! Local persistence: settings, credentials, queue restore, Feishin import and the cover art cache.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

pub mod cover_cache;
pub mod credentials;
pub mod feishin_import;
pub mod queue_store;
pub mod settings;

pub const APP_DIR_NAME: &str = "feishin-rs";

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
        Self::from_lookup(|name| std::env::var_os(name))
    }

    /// Same as [`Paths::from_env`] with a custom environment lookup.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<OsString>) -> Result<Self> {
        Ok(Self {
            config_dir: xdg_base(&lookup, "XDG_CONFIG_HOME", ".config")?.join(APP_DIR_NAME),
            cache_dir: xdg_base(&lookup, "XDG_CACHE_HOME", ".cache")?.join(APP_DIR_NAME),
            data_dir: xdg_base(&lookup, "XDG_DATA_HOME", ".local/share")?.join(APP_DIR_NAME),
        })
    }

    /// All three directories under one root, handy for tests and portable setups.
    pub fn under(root: &Path) -> Self {
        Self {
            config_dir: root.join("config"),
            cache_dir: root.join("cache"),
            data_dir: root.join("data"),
        }
    }
}

/// Resolves an XDG base directory (`XDG_CONFIG_HOME` style); relative or empty values are ignored per the spec.
pub(crate) fn xdg_base(
    lookup: &impl Fn(&str) -> Option<OsString>,
    var: &str,
    home_fallback: &str,
) -> Result<PathBuf> {
    if let Some(value) = lookup(var).map(PathBuf::from)
        && value.is_absolute()
    {
        return Ok(value);
    }
    let home = lookup("HOME")
        .map(PathBuf::from)
        .filter(|home| home.is_absolute())
        .ok_or_else(|| StorageError::Invalid(format!("neither {var} nor HOME is set")))?;
    Ok(home.join(home_fallback))
}

/// Writes `bytes` to `path` through a temp file in the same directory and a rename.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let mut tmp = tempfile::Builder::new()
        .prefix(".tmp-")
        .suffix(".part")
        .tempfile_in(dir)?;
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| StorageError::Io(e.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn lookup(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let map: HashMap<String, OsString> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), OsString::from(v)))
            .collect();
        move |name| map.get(name).cloned()
    }

    #[test]
    fn home_fallbacks() {
        let paths = Paths::from_lookup(lookup(&[("HOME", "/home/u")])).unwrap();
        assert_eq!(
            paths.config_dir,
            PathBuf::from("/home/u/.config/feishin-rs")
        );
        assert_eq!(paths.cache_dir, PathBuf::from("/home/u/.cache/feishin-rs"));
        assert_eq!(
            paths.data_dir,
            PathBuf::from("/home/u/.local/share/feishin-rs")
        );
    }

    #[test]
    fn xdg_vars_win() {
        let paths = Paths::from_lookup(lookup(&[
            ("HOME", "/home/u"),
            ("XDG_CONFIG_HOME", "/cfg"),
            ("XDG_CACHE_HOME", "/cache"),
            ("XDG_DATA_HOME", "/data"),
        ]))
        .unwrap();
        assert_eq!(paths.config_dir, PathBuf::from("/cfg/feishin-rs"));
        assert_eq!(paths.cache_dir, PathBuf::from("/cache/feishin-rs"));
        assert_eq!(paths.data_dir, PathBuf::from("/data/feishin-rs"));
    }

    #[test]
    fn relative_or_empty_xdg_is_ignored() {
        let paths = Paths::from_lookup(lookup(&[
            ("HOME", "/home/u"),
            ("XDG_CONFIG_HOME", "relative"),
            ("XDG_CACHE_HOME", ""),
        ]))
        .unwrap();
        assert_eq!(
            paths.config_dir,
            PathBuf::from("/home/u/.config/feishin-rs")
        );
        assert_eq!(paths.cache_dir, PathBuf::from("/home/u/.cache/feishin-rs"));
    }

    #[test]
    fn missing_home_errors() {
        assert!(matches!(
            Paths::from_lookup(lookup(&[])),
            Err(StorageError::Invalid(_))
        ));
        let paths = Paths::from_lookup(lookup(&[
            ("XDG_CONFIG_HOME", "/c"),
            ("XDG_CACHE_HOME", "/k"),
        ]));
        assert!(paths.is_err());
    }

    #[test]
    fn from_env_does_not_fail_with_home() {
        if std::env::var_os("HOME").is_some() {
            let paths = Paths::from_env().unwrap();
            assert!(paths.config_dir.ends_with(APP_DIR_NAME));
        }
    }

    #[test]
    fn write_atomic_replaces_and_leaves_no_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/file.txt");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        let entries: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(entries, vec![OsString::from("file.txt")]);
    }
}
