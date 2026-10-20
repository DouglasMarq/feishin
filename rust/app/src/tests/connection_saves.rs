use std::sync::Arc;

use anyhow::{Result, anyhow};
use feishin_api::Credential;
use feishin_core::model::{ServerConfig, ServerType};
use feishin_storage::feishin_import::ImportedServer;
use feishin_storage::queue_store::{EncodedQueue, PlaybackState, StoredQueue};
use feishin_storage::settings::{IntegrationSettings, Settings};
use tokio::sync::mpsc;

use super::harness::{RecordingAudio, RecordingUi, start_harness};
use crate::controller::{Command, Controller, Deps};
use crate::demo::{FakeServer, demo_credential};
use crate::media::NoMediaSession;
use crate::session::Session;
use crate::store::{JsonStore, StateFile, Store};

const DISK_FULL: &str = "the disk is full";

fn subsonic_server(id: &str) -> ServerConfig {
    ServerConfig {
        id: id.to_owned(),
        name: format!("Server {id}"),
        server_type: ServerType::Subsonic,
        url: format!("http://{id}.navidrome.example"),
        username: "feishin".into(),
    }
}

fn subsonic_credential(token: &str) -> Credential {
    Credential::Subsonic {
        subsonic: token.to_owned(),
    }
}

struct FullDisk(JsonStore);

impl Store for FullDisk {
    fn load_settings(&self) -> Result<Settings> {
        self.0.load_settings()
    }
    fn save_settings(&self, _settings: &Settings) -> Result<()> {
        Err(anyhow!(DISK_FULL))
    }
    fn load_queue(&self) -> Result<Option<StoredQueue>> {
        self.0.load_queue()
    }
    fn save_queue(&self, queue: &EncodedQueue) -> Result<()> {
        self.0.save_queue(queue)
    }
    fn save_playback(&self, playback: &PlaybackState) -> Result<()> {
        self.0.save_playback(playback)
    }
    fn credential(&self, server_id: &str) -> Result<Option<Credential>> {
        self.0.credential(server_id)
    }
    fn set_credential(&self, _server_id: &str, _credential: &Credential) -> Result<()> {
        Err(anyhow!(DISK_FULL))
    }
    fn delete_credential(&self, _server_id: &str) -> Result<()> {
        Err(anyhow!(DISK_FULL))
    }
    fn import_feishin(&self) -> Result<Vec<ImportedServer>> {
        self.0.import_feishin()
    }
    fn read_state(&self, file: StateFile) -> Result<Option<Vec<u8>>> {
        self.0.read_state(file)
    }
    fn write_state(&self, file: StateFile, bytes: &[u8]) -> Result<()> {
        self.0.write_state(file, bytes)
    }
}

fn controller_on_a_full_disk(directory: &tempfile::TempDir) -> (Controller, Arc<RecordingUi>) {
    let session = Session::default();
    session.set(Some(Arc::new(FakeServer::new(10))));
    let ui = Arc::new(RecordingUi::default());
    let (commands, _ignored) = mpsc::unbounded_channel();
    let deps = Deps {
        audio: Box::new(RecordingAudio::default()),
        media: Box::new(NoMediaSession),
        store: Arc::new(FullDisk(JsonStore::new(directory.path()))),
        session,
        ui: ui.clone(),
        commands,
        covers: Arc::new(crate::covers::DemoCovers),
    };
    (
        Controller::new(deps, Settings::default(), None, Some(1)),
        ui,
    )
}

#[tokio::test]
async fn given_connected_the_controller_saves_the_credential() {
    let mut harness = start_harness(Settings::default());
    let config = subsonic_server("a");
    harness.controller.handle(Command::Connected {
        config: config.clone(),
        credential: Some(subsonic_credential("t=a")),
        others: Vec::new(),
    });
    assert_eq!(
        harness.store.credential(&config.id).unwrap(),
        Some(subsonic_credential("t=a"))
    );
}

#[tokio::test]
async fn given_imported_servers_the_controller_saves_their_credentials() {
    let mut harness = start_harness(Settings::default());
    let imported = vec![
        ImportedServer {
            config: subsonic_server("b"),
            credential: Some(subsonic_credential("t=b")),
        },
        ImportedServer {
            config: subsonic_server("c"),
            credential: None,
        },
    ];
    harness.controller.handle(Command::Connected {
        config: subsonic_server("b"),
        credential: None,
        others: imported,
    });
    let store = &harness.store;
    assert_eq!(
        store.credential("b").unwrap(),
        Some(subsonic_credential("t=b"))
    );
    assert_eq!(store.credential("c").unwrap(), None);
    let saved = store.load_settings().unwrap();
    let ids: Vec<&str> = saved
        .servers
        .iter()
        .map(|server| server.id.as_str())
        .collect();
    assert_eq!(ids, ["c", "b"]);
    assert_eq!(saved.current_server_id.as_deref(), Some("b"));
}

#[tokio::test]
async fn given_a_failed_credential_save_the_status_shows_an_error() {
    let directory = tempfile::tempdir().unwrap();
    let (mut controller, ui) = controller_on_a_full_disk(&directory);
    ui.clear();
    controller.handle(Command::Connected {
        config: subsonic_server("a"),
        credential: Some(demo_credential()),
        others: Vec::new(),
    });
    assert!(ui.errors().contains(&format!(
        "Could not save the login for Server a: {DISK_FULL}"
    )));
}

#[tokio::test]
async fn given_a_failed_settings_save_the_status_shows_an_error() {
    let directory = tempfile::tempdir().unwrap();
    let (mut controller, ui) = controller_on_a_full_disk(&directory);
    ui.clear();
    controller.handle(Command::SaveIntegrations(IntegrationSettings::default()));
    assert_eq!(
        ui.errors(),
        [format!("Could not save the settings: {DISK_FULL}")]
    );
}

#[tokio::test]
async fn given_a_failed_login_delete_on_server_removal_the_status_shows_an_error() {
    let directory = tempfile::tempdir().unwrap();
    let (mut controller, ui) = controller_on_a_full_disk(&directory);
    ui.clear();
    controller.handle(Command::RemoveServer("a".into()));
    assert!(
        ui.errors()
            .contains(&format!("Could not delete the saved login: {DISK_FULL}"))
    );
}

#[tokio::test]
async fn given_saved_settings_the_session_holds_the_same_snapshot() {
    let mut harness = start_harness(Settings::default());
    let integrations = IntegrationSettings {
        lyrics_sources: vec!["genius".into()],
        ..IntegrationSettings::default()
    };
    harness
        .controller
        .handle(Command::SaveIntegrations(integrations.clone()));
    assert_eq!(harness.session.settings().integrations, integrations);
    assert_eq!(
        *harness.session.settings(),
        harness.store.load_settings().unwrap()
    );
}
