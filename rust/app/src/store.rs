use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result, anyhow};
use feishin_api::Credential;
use feishin_storage::Paths;
use feishin_storage::credentials;
use feishin_storage::feishin_import::{self, ImportedServer};
use feishin_storage::queue_store::{self, StoredQueue};
use feishin_storage::settings::Settings;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Persistence used by the app (`feishin_storage` in real mode).
pub trait Store: Send + Sync {
    fn load_settings(&self) -> Result<Settings>;
    fn save_settings(&self, settings: &Settings) -> Result<()>;
    fn load_queue(&self) -> Result<Option<StoredQueue>>;
    fn save_queue(&self, queue: &StoredQueue) -> Result<()>;
    fn credential(&self, server_id: &str) -> Result<Option<Credential>>;
    fn set_credential(&self, server_id: &str, credential: &Credential) -> Result<()>;
    fn import_feishin(&self) -> Result<Vec<ImportedServer>>;
}

pub struct SystemStore {
    paths: Paths,
}

impl SystemStore {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            paths: Paths::from_env()?,
        })
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }
}

fn feishin_config_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".config")))
        .map(|dir| dir.join("feishin"))
}

impl Store for SystemStore {
    fn load_settings(&self) -> Result<Settings> {
        Ok(Settings::load(&self.paths)?)
    }
    fn save_settings(&self, settings: &Settings) -> Result<()> {
        Ok(settings.save(&self.paths)?)
    }
    fn load_queue(&self) -> Result<Option<StoredQueue>> {
        Ok(queue_store::load(&self.paths)?)
    }
    fn save_queue(&self, queue: &StoredQueue) -> Result<()> {
        Ok(queue_store::save(&self.paths, queue)?)
    }
    fn credential(&self, server_id: &str) -> Result<Option<Credential>> {
        match credentials::get(server_id)? {
            Some(json) => Ok(Some(Credential::from_json(&json)?)),
            None => Ok(None),
        }
    }
    fn set_credential(&self, server_id: &str, credential: &Credential) -> Result<()> {
        Ok(credentials::set(server_id, &credential.to_json())?)
    }
    fn import_feishin(&self) -> Result<Vec<ImportedServer>> {
        let dir = feishin_config_dir().ok_or_else(|| anyhow!("cannot locate ~/.config"))?;
        Ok(feishin_import::import_servers(&dir)?)
    }
}

/// JSON files in one directory and in-memory credentials; used by `--demo` and tests.
pub struct JsonStore {
    dir: PathBuf,
    credentials: Mutex<HashMap<String, Credential>>,
}

impl JsonStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            credentials: Mutex::default(),
        }
    }

    fn read<T: DeserializeOwned>(&self, name: &str) -> Result<Option<T>> {
        let path = self.dir.join(name);
        match std::fs::read(&path) {
            Ok(bytes) => Ok(Some(
                serde_json::from_slice(&bytes).with_context(|| path.display().to_string())?,
            )),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn write<T: Serialize>(&self, name: &str, value: &T) -> Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let tmp = self.dir.join(format!("{name}.tmp"));
        std::fs::write(&tmp, serde_json::to_vec(value)?)?;
        std::fs::rename(&tmp, self.dir.join(name))?;
        Ok(())
    }
}

impl Store for JsonStore {
    fn load_settings(&self) -> Result<Settings> {
        Ok(self.read("settings.json")?.unwrap_or_default())
    }
    fn save_settings(&self, settings: &Settings) -> Result<()> {
        self.write("settings.json", settings)
    }
    fn load_queue(&self) -> Result<Option<StoredQueue>> {
        self.read("queue.json")
    }
    fn save_queue(&self, queue: &StoredQueue) -> Result<()> {
        self.write("queue.json", queue)
    }
    fn credential(&self, server_id: &str) -> Result<Option<Credential>> {
        let credentials = self
            .credentials
            .lock()
            .map_err(|_| anyhow!("credential store poisoned"))?;
        Ok(credentials.get(server_id).cloned())
    }
    fn set_credential(&self, server_id: &str, credential: &Credential) -> Result<()> {
        self.credentials
            .lock()
            .map_err(|_| anyhow!("credential store poisoned"))?
            .insert(server_id.to_owned(), credential.clone());
        Ok(())
    }
    fn import_feishin(&self) -> Result<Vec<ImportedServer>> {
        Ok(Vec::new())
    }
}
