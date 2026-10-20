use std::sync::{Arc, Mutex, PoisonError};

use feishin_remote::{DeviceId, DeviceInfo, PairingToken, TrustStore, new_device_id};
use serde::{Deserialize, Serialize};

use crate::store::{StateFile, Store};

const BOOK_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TrustedDevice {
    id: DeviceId,
    name: String,
    token: PairingToken,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct KnownHost {
    id: DeviceId,
    token: PairingToken,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct DeviceBook {
    version: u32,
    device_id: DeviceId,
    trusted: Vec<TrustedDevice>,
    hosts: Vec<KnownHost>,
}

impl DeviceBook {
    fn new() -> Self {
        Self {
            version: BOOK_VERSION,
            device_id: DeviceId(new_device_id()),
            trusted: Vec::new(),
            hosts: Vec::new(),
        }
    }
}

pub struct FileTrust {
    store: Arc<dyn Store>,
    book: Mutex<DeviceBook>,
    persists: bool,
}

impl FileTrust {
    pub fn load(store: Arc<dyn Store>) -> Self {
        let saved = read_book(store.as_ref());
        let trust = Self {
            book: Mutex::new(saved.clone().unwrap_or_else(DeviceBook::new)),
            store,
            persists: true,
        };
        if saved.is_none() {
            trust.save();
        }
        trust
    }

    pub fn ephemeral(store: Arc<dyn Store>) -> Self {
        Self {
            store,
            book: Mutex::new(DeviceBook::new()),
            persists: false,
        }
    }

    pub fn device_id(&self) -> DeviceId {
        self.read(|book| book.device_id.clone())
    }

    pub fn forget_trusted(&self) {
        self.change(|book| book.trusted.clear());
    }

    fn read<Output>(&self, reading: impl FnOnce(&DeviceBook) -> Output) -> Output {
        reading(&self.book.lock().unwrap_or_else(PoisonError::into_inner))
    }

    fn change(&self, changing: impl FnOnce(&mut DeviceBook)) {
        changing(&mut self.book.lock().unwrap_or_else(PoisonError::into_inner));
        self.save();
    }

    fn save(&self) {
        if !self.persists {
            return;
        }
        let bytes = match self.read(serde_json::to_vec) {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::warn!("encoding the approved devices failed: {error}");
                return;
            }
        };
        if let Err(error) = self.store.write_state(StateFile::RemoteDevices, &bytes) {
            tracing::warn!("saving the approved devices failed: {error:#}");
        }
    }
}

fn read_book(store: &dyn Store) -> Option<DeviceBook> {
    let bytes = store
        .read_state(StateFile::RemoteDevices)
        .inspect_err(|error| tracing::warn!("reading the approved devices failed: {error:#}"))
        .ok()??;
    serde_json::from_slice::<DeviceBook>(&bytes)
        .inspect_err(|error| tracing::warn!("the approved devices file is unreadable: {error}"))
        .ok()
        .filter(|book| book.version == BOOK_VERSION)
}

impl TrustStore for FileTrust {
    fn trusted_token(&self, device: &DeviceId) -> Option<PairingToken> {
        self.read(|book| {
            book.trusted
                .iter()
                .find(|trusted| trusted.id == *device)
                .map(|trusted| trusted.token)
        })
    }

    fn trust(&self, device: &DeviceInfo, token: PairingToken) {
        self.change(|book| {
            book.trusted.retain(|trusted| trusted.id != device.id);
            book.trusted.push(TrustedDevice {
                id: device.id.clone(),
                name: device.name.clone(),
                token,
            });
        });
    }

    fn host_token(&self, host: &DeviceId) -> Option<PairingToken> {
        self.read(|book| {
            book.hosts
                .iter()
                .find(|known| known.id == *host)
                .map(|known| known.token)
        })
    }

    fn remember_host(&self, host: &DeviceId, token: PairingToken) {
        self.change(|book| {
            book.hosts.retain(|known| known.id != *host);
            book.hosts.push(KnownHost {
                id: host.clone(),
                token,
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::JsonStore;

    fn device(id: &str) -> DeviceInfo {
        DeviceInfo {
            id: DeviceId(id.into()),
            name: id.to_uppercase(),
        }
    }

    #[test]
    fn the_identity_and_approvals_survive_a_restart() {
        let directory = tempfile::tempdir().unwrap();
        let store: Arc<dyn Store> = Arc::new(JsonStore::new(directory.path()));
        let first = FileTrust::load(Arc::clone(&store));
        let token = PairingToken::random();
        first.trust(&device("phone"), token);
        first.remember_host(&DeviceId("tv".into()), token);
        let again = FileTrust::load(store);
        assert_eq!(again.device_id(), first.device_id());
        assert_eq!(again.trusted_token(&DeviceId("phone".into())), Some(token));
        assert_eq!(again.host_token(&DeviceId("tv".into())), Some(token));
        again.forget_trusted();
        assert_eq!(again.trusted_token(&DeviceId("phone".into())), None);
        assert_eq!(again.host_token(&DeviceId("tv".into())), Some(token));
    }

    #[test]
    fn an_ephemeral_identity_is_new_every_time_and_never_saved() {
        let directory = tempfile::tempdir().unwrap();
        let store: Arc<dyn Store> = Arc::new(JsonStore::new(directory.path()));
        let first = FileTrust::ephemeral(Arc::clone(&store));
        first.trust(&device("phone"), PairingToken::random());
        assert_ne!(
            FileTrust::ephemeral(Arc::clone(&store)).device_id(),
            first.device_id()
        );
        assert!(!directory.path().join("remote-devices.json").exists());
    }
}
