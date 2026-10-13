use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use anyhow::{Context, Result, anyhow};
use feishin_api::Credential;
use feishin_storage::Paths;
use feishin_storage::credentials;
use feishin_storage::feishin_import::{self, ImportedServer};
use feishin_storage::queue_store::{self, EncodedQueue, PlaybackState, StoredQueue};
use feishin_storage::settings::Settings;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub trait Store: Send + Sync {
    fn load_settings(&self) -> Result<Settings>;
    fn save_settings(&self, settings: &Settings) -> Result<()>;
    fn load_queue(&self) -> Result<Option<StoredQueue>>;
    fn save_queue(&self, queue: &EncodedQueue) -> Result<()>;
    fn save_playback(&self, playback: &PlaybackState) -> Result<()>;
    fn credential(&self, server_id: &str) -> Result<Option<Credential>>;
    fn set_credential(&self, server_id: &str, credential: &Credential) -> Result<()>;
    fn delete_credential(&self, server_id: &str) -> Result<()>;
    fn import_feishin(&self) -> Result<Vec<ImportedServer>>;
}

struct QueueFiles {
    data_dir: PathBuf,
    access: Mutex<()>,
}

impl QueueFiles {
    fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            access: Mutex::new(()),
        }
    }

    fn exclusively<Output>(
        &self,
        action: impl FnOnce(&Path) -> feishin_storage::Result<Output>,
    ) -> Result<Output> {
        let access = self.access.lock().unwrap_or_else(PoisonError::into_inner);
        let outcome = action(&self.data_dir);
        drop(access);
        Ok(outcome?)
    }

    fn load(&self) -> Result<Option<StoredQueue>> {
        self.exclusively(queue_store::load)
    }

    fn save(&self, queue: &EncodedQueue) -> Result<()> {
        self.exclusively(|data_dir| queue_store::save(data_dir, queue))
    }

    fn save_playback(&self, playback: &PlaybackState) -> Result<()> {
        self.exclusively(|data_dir| queue_store::save_playback(data_dir, playback))
    }
}

pub struct SystemStore {
    paths: Paths,
    queue_files: QueueFiles,
}

impl SystemStore {
    pub fn from_env() -> Result<Self> {
        let paths = Paths::from_env()?;
        Ok(Self {
            queue_files: QueueFiles::new(paths.data_dir.clone()),
            paths,
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
        self.queue_files.load()
    }
    fn save_queue(&self, queue: &EncodedQueue) -> Result<()> {
        self.queue_files.save(queue)
    }
    fn save_playback(&self, playback: &PlaybackState) -> Result<()> {
        self.queue_files.save_playback(playback)
    }
    fn credential(&self, server_id: &str) -> Result<Option<Credential>> {
        Ok(credentials::get(server_id)?
            .map(|json| Credential::from_json(&json))
            .transpose()?)
    }
    fn set_credential(&self, server_id: &str, credential: &Credential) -> Result<()> {
        Ok(credentials::set(server_id, &credential.to_json())?)
    }
    fn delete_credential(&self, server_id: &str) -> Result<()> {
        Ok(credentials::delete(server_id)?)
    }
    fn import_feishin(&self) -> Result<Vec<ImportedServer>> {
        let dir = feishin_config_dir().ok_or_else(|| anyhow!("cannot locate ~/.config"))?;
        Ok(feishin_import::import_servers(&dir)?)
    }
}

pub struct JsonStore {
    dir: PathBuf,
    queue_files: QueueFiles,
    credentials: Mutex<HashMap<String, Credential>>,
}

impl JsonStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        Self {
            queue_files: QueueFiles::new(dir.clone()),
            dir,
            credentials: Mutex::default(),
        }
    }

    fn read<T: DeserializeOwned>(&self, name: &str) -> Result<Option<T>> {
        let path = self.dir.join(name);
        match std::fs::read(&path) {
            Ok(bytes) => Ok(Some(
                serde_json::from_slice(&bytes).with_context(|| path.display().to_string())?,
            )),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn write<T: Serialize>(&self, name: &str, value: &T) -> Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let staging_path = self.dir.join(format!("{name}.tmp"));
        std::fs::write(&staging_path, serde_json::to_vec(value)?)?;
        std::fs::rename(&staging_path, self.dir.join(name))?;
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
        self.queue_files.load()
    }
    fn save_queue(&self, queue: &EncodedQueue) -> Result<()> {
        self.queue_files.save(queue)
    }
    fn save_playback(&self, playback: &PlaybackState) -> Result<()> {
        self.queue_files.save_playback(playback)
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
    fn delete_credential(&self, server_id: &str) -> Result<()> {
        self.credentials
            .lock()
            .map_err(|_| anyhow!("credential store poisoned"))?
            .remove(server_id);
        Ok(())
    }
    fn import_feishin(&self) -> Result<Vec<ImportedServer>> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use feishin_core::queue::RepeatMode;

    use super::*;

    #[test]
    fn queue_files_are_touched_by_one_caller_at_a_time() {
        let directory = tempfile::tempdir().unwrap();
        let queue_files = Arc::new(QueueFiles::new(directory.path().to_path_buf()));
        let playback_file = directory.path().join(queue_store::PLAYBACK_FILE);
        let held = queue_files.access.lock().unwrap();
        let saver = {
            let queue_files = Arc::clone(&queue_files);
            std::thread::spawn(move || {
                queue_files.save_playback(&PlaybackState {
                    current_uid: Some(3),
                    position_ms: 10,
                    repeat: RepeatMode::None,
                })
            })
        };
        std::thread::sleep(Duration::from_millis(50));
        assert!(!playback_file.exists());
        drop(held);
        assert!(saver.join().unwrap().is_ok());
        assert!(playback_file.exists());
    }
}
