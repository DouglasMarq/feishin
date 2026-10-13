//! Imports servers saved by the Electron Feishin (zustand `store_authentication` in Chromium
//! localStorage under `~/.config/feishin/Local Storage/leveldb`).

use std::path::Path;

use feishin_core::model::ServerConfig;

use crate::Result;

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedServer {
    pub config: ServerConfig,
    /// Credential in the format the matching backend's `with_credential` accepts, when Feishin saved one.
    pub credential: Option<String>,
}

/// `feishin_config_dir` is usually `~/.config/feishin`.
pub fn import_servers(_feishin_config_dir: &Path) -> Result<Vec<ImportedServer>> {
    todo!()
}
