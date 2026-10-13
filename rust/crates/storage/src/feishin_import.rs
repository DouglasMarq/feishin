//! Imports servers saved by the Electron Feishin (zustand `store_authentication` in Chromium
//! localStorage under `~/.config/feishin/Local Storage/leveldb`).

use std::path::Path;

use feishin_api::Credential;
use feishin_core::model::ServerConfig;

use crate::Result;

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedServer {
    pub config: ServerConfig,
    /// Ready for `feishin_storage::credentials::set(id, &credential.to_json())`, when Feishin saved one.
    pub credential: Option<Credential>,
}

/// `feishin_config_dir` is usually `~/.config/feishin`.
pub fn import_servers(_feishin_config_dir: &Path) -> Result<Vec<ImportedServer>> {
    todo!()
}
