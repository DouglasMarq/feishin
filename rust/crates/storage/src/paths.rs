use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::{Result, StorageError};

pub const APP_DIR_NAME: &str = "feishin-rs";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub data_dir: PathBuf,
}

impl Paths {
    pub fn from_env() -> Result<Self> {
        Self::from_lookup(|variable_name| std::env::var_os(variable_name))
    }

    pub fn from_lookup(lookup: impl Fn(&str) -> Option<OsString>) -> Result<Self> {
        Ok(Self {
            config_dir: xdg_base(&lookup, "XDG_CONFIG_HOME", ".config")?.join(APP_DIR_NAME),
            cache_dir: xdg_base(&lookup, "XDG_CACHE_HOME", ".cache")?.join(APP_DIR_NAME),
            data_dir: xdg_base(&lookup, "XDG_DATA_HOME", ".local/share")?.join(APP_DIR_NAME),
        })
    }

    pub fn under(root: &Path) -> Self {
        Self {
            config_dir: root.join("config"),
            cache_dir: root.join("cache"),
            data_dir: root.join("data"),
        }
    }
}

pub(crate) fn xdg_base(
    lookup: &impl Fn(&str) -> Option<OsString>,
    variable_name: &str,
    home_fallback: &str,
) -> Result<PathBuf> {
    match absolute_path_from(lookup, variable_name) {
        Some(base) => Ok(base),
        None => home_directory(lookup, variable_name).map(|home| home.join(home_fallback)),
    }
}

fn home_directory(
    lookup: &impl Fn(&str) -> Option<OsString>,
    variable_name: &str,
) -> Result<PathBuf> {
    absolute_path_from(lookup, "HOME")
        .ok_or_else(|| StorageError::Invalid(format!("neither {variable_name} nor HOME is set")))
}

fn absolute_path_from(
    lookup: &impl Fn(&str) -> Option<OsString>,
    variable_name: &str,
) -> Option<PathBuf> {
    lookup(variable_name)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn lookup(variables: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let environment: HashMap<String, OsString> = variables
            .iter()
            .map(|(name, value)| (name.to_string(), OsString::from(value)))
            .collect();
        move |name| environment.get(name).cloned()
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
}
