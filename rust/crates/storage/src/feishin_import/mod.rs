mod local_storage;
mod raw_store;
mod server_mapping;

use std::path::{Path, PathBuf};

use feishin_api::Credential;
use feishin_core::model::ServerConfig;

use crate::{Result, StorageError, xdg_base};
use local_storage::read_store_value;
use raw_store::{Persisted, RawServerEntry};
use server_mapping::map_server;

pub const STORE_KEY: &str = "store_authentication";

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedServer {
    pub config: ServerConfig,
    pub credential: Option<Credential>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FeishinImport {
    pub servers: Vec<ImportedServer>,
    pub current_server_id: Option<String>,
}

pub fn default_config_dir() -> Result<PathBuf> {
    let config_home = xdg_base(
        &|variable_name| std::env::var_os(variable_name),
        "XDG_CONFIG_HOME",
        ".config",
    )?;
    Ok(config_home.join("feishin"))
}

pub fn import_servers(feishin_config_dir: &Path) -> Result<Vec<ImportedServer>> {
    Ok(import(feishin_config_dir)?.servers)
}

pub fn import(feishin_config_dir: &Path) -> Result<FeishinImport> {
    let leveldb_dir = feishin_config_dir.join("Local Storage").join("leveldb");
    if !leveldb_dir.is_dir() {
        return Ok(FeishinImport::default());
    }
    match read_store_value(&leveldb_dir)? {
        Some(json) => parse_auth_store(&json),
        None => Ok(FeishinImport::default()),
    }
}

pub fn parse_auth_store(json: &str) -> Result<FeishinImport> {
    let persisted: Persisted = serde_json::from_str(json)
        .map_err(|error| StorageError::Invalid(format!("{STORE_KEY}: {error}")))?;
    let servers: Vec<ImportedServer> = persisted
        .state
        .server_list
        .into_entries()
        .into_iter()
        .filter_map(import_server_entry)
        .collect();
    let current_server_id = persisted
        .state
        .current_server
        .and_then(|current| current.id)
        .filter(|id| servers.iter().any(|server| &server.config.id == id));
    Ok(FeishinImport {
        servers,
        current_server_id,
    })
}

fn import_server_entry(entry: RawServerEntry) -> Option<ImportedServer> {
    let RawServerEntry { key, value } = entry;
    match serde_json::from_value(value) {
        Ok(raw_server) => map_server(&key, raw_server),
        Err(error) => {
            tracing::warn!("skipping Feishin server {key}: {error}");
            None
        }
    }
}

#[cfg(test)]
mod tests;
