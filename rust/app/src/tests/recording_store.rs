use std::sync::Arc;

use anyhow::Result;
use feishin_api::Credential;
use feishin_storage::feishin_import::ImportedServer;
use feishin_storage::queue_store::{EncodedQueue, PlaybackState, StoredQueue};
use feishin_storage::settings::Settings;
use tokio::sync::mpsc;

use crate::store::{JsonStore, StateFile, Store};

pub(super) struct RecordingStore {
    inner: Arc<JsonStore>,
    saved: mpsc::UnboundedSender<()>,
}

impl RecordingStore {
    pub(super) fn new(inner: Arc<JsonStore>) -> (Self, mpsc::UnboundedReceiver<()>) {
        let (saved, saves) = mpsc::unbounded_channel();
        (Self { inner, saved }, saves)
    }

    fn report_saved(&self, saved: Result<()>) -> Result<()> {
        self.saved.send(()).ok();
        saved
    }
}

impl Store for RecordingStore {
    fn load_settings(&self) -> Result<Settings> {
        self.inner.load_settings()
    }
    fn save_settings(&self, settings: &Settings) -> Result<()> {
        self.inner.save_settings(settings)
    }
    fn load_queue(&self) -> Result<Option<StoredQueue>> {
        self.inner.load_queue()
    }
    fn save_queue(&self, queue: &EncodedQueue) -> Result<()> {
        self.report_saved(self.inner.save_queue(queue))
    }
    fn save_playback(&self, playback: &PlaybackState) -> Result<()> {
        self.report_saved(self.inner.save_playback(playback))
    }
    fn credential(&self, server_id: &str) -> Result<Option<Credential>> {
        self.inner.credential(server_id)
    }
    fn set_credential(&self, server_id: &str, credential: &Credential) -> Result<()> {
        self.inner.set_credential(server_id, credential)
    }
    fn delete_credential(&self, server_id: &str) -> Result<()> {
        self.inner.delete_credential(server_id)
    }
    fn import_feishin(&self) -> Result<Vec<ImportedServer>> {
        self.inner.import_feishin()
    }
    fn read_state(&self, file: StateFile) -> Result<Option<Vec<u8>>> {
        self.inner.read_state(file)
    }
    fn write_state(&self, file: StateFile, bytes: &[u8]) -> Result<()> {
        self.inner.write_state(file, bytes)
    }
}
