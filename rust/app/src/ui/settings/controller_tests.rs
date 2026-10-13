use std::sync::{Arc, Mutex};

use anyhow::Result;
use feishin_core::queue::Uid;
use feishin_player::{AudioDevice, PlayerEvent};
use feishin_storage::settings::{
    IntegrationSettings, PlaybackSettings, ReplayGainSetting, Settings, UiSettings,
};
use tokio::sync::{mpsc, oneshot};

use super::test_support::{device, server, two_servers};
use crate::audio::{AudioOutput, Track};
use crate::controller::{Command, Controller, Deps, UiSink, UiUpdate};
use crate::demo::{FakeServer, demo_credential, test_song};
use crate::media::NoMediaSession;
use crate::session::Session;
use crate::store::{JsonStore, Store};

#[derive(Clone, Default)]
struct RecordingAudio(Arc<Mutex<Vec<String>>>);

impl RecordingAudio {
    fn log(&self, call: String) -> Result<()> {
        if let Ok(mut calls) = self.0.lock() {
            calls.push(call);
        }
        Ok(())
    }

    fn take(&self) -> Vec<String> {
        self.0
            .lock()
            .map(|mut calls| calls.drain(..).collect())
            .unwrap_or_default()
    }
}

impl AudioOutput for RecordingAudio {
    fn load(&self, track: &Track, _start_ms: u64) -> Result<()> {
        self.log(format!("load {}", track.uid))
    }
    fn preload_next(&self, track: &Track) -> Result<()> {
        self.log(format!("preload {}", track.uid))
    }
    fn clear_next(&self) -> Result<()> {
        self.log("clear".into())
    }
    fn play(&self) -> Result<()> {
        Ok(())
    }
    fn pause(&self) -> Result<()> {
        Ok(())
    }
    fn stop(&self) -> Result<()> {
        self.log("stop".into())
    }
    fn seek(&self, _position_ms: u64) -> Result<()> {
        Ok(())
    }
    fn set_volume(&self, _volume: u8) -> Result<()> {
        Ok(())
    }
    fn set_muted(&self, _muted: bool) -> Result<()> {
        Ok(())
    }
    fn set_replay_gain(&self, mode: ReplayGainSetting) -> Result<()> {
        self.log(format!("replay gain {mode:?}"))
    }
    fn set_audio_device(&self, device: Option<&str>) -> Result<()> {
        self.log(format!("device {}", device.unwrap_or("auto")))
    }
    fn audio_devices(&self) -> Result<Vec<AudioDevice>> {
        Ok(vec![device("auto", "Autoselect device")])
    }
}

struct NoUi;

impl UiSink for NoUi {
    fn send(&self, _update: UiUpdate) {}
}

struct Harness {
    controller: Controller,
    audio: RecordingAudio,
    store: Arc<JsonStore>,
    session: Session,
    _dir: tempfile::TempDir,
}

fn harness(settings: Settings) -> Harness {
    let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir: {error}"));
    let store = Arc::new(JsonStore::new(dir.path()));
    let session = Session::default();
    session.set(Some(Arc::new(FakeServer::new(10))));
    let audio = RecordingAudio::default();
    let deps = Deps {
        audio: Box::new(audio.clone()),
        media: Box::new(NoMediaSession),
        store: store.clone(),
        session: session.clone(),
        ui: Arc::new(NoUi),
        commands: mpsc::unbounded_channel().0,
    };
    Harness {
        controller: Controller::new(deps, settings, None, Some(7)),
        audio,
        store,
        session,
        _dir: dir,
    }
}

impl Harness {
    fn start(&mut self) {
        let songs = vec![test_song("a", 100_000), test_song("b", 100_000)];
        self.controller.play_now(songs, "a");
        let uid = self
            .controller
            .queue()
            .current()
            .map_or(Uid::MAX, |item| item.uid);
        self.controller
            .handle(Command::Player(PlayerEvent::Started { uid }));
        self.audio.take();
    }

    fn saved(&self) -> Settings {
        self.store.load_settings().unwrap_or_default()
    }
}

#[tokio::test]
async fn applying_playback_reaches_the_player_and_is_saved() {
    let mut harness = harness(Settings::default());
    harness.start();
    let next = harness
        .controller
        .queue()
        .peek_next()
        .map_or(Uid::MAX, |item| item.uid);
    harness
        .controller
        .handle(Command::ApplyPlayback(PlaybackSettings {
            replay_gain: ReplayGainSetting::Album,
            audio_device: Some("pipewire/hdmi".into()),
            max_bitrate_kbps: Some(128),
            volume: 5,
            ..PlaybackSettings::default()
        }));
    assert_eq!(
        harness.audio.take(),
        [
            "replay gain Album".to_owned(),
            "device pipewire/hdmi".into(),
            "clear".into(),
            format!("preload {next}"),
        ]
    );
    let saved = harness.saved().playback;
    assert_eq!(saved.replay_gain, ReplayGainSetting::Album);
    assert_eq!(saved.max_bitrate_kbps, Some(128));
    assert_eq!(saved.volume, PlaybackSettings::default().volume);

    harness
        .controller
        .handle(Command::ApplyPlayback(PlaybackSettings {
            gapless: false,
            ..saved
        }));
    assert_eq!(harness.audio.take(), ["clear"]);
}

#[tokio::test]
async fn removing_servers_forgets_logins_and_disconnects_the_current_one() {
    let mut harness = harness(two_servers());
    for id in ["demo", "b"] {
        harness.store.set_credential(id, &demo_credential()).ok();
    }
    harness.start();

    harness.controller.handle(Command::RemoveServer("b".into()));
    assert!(harness.store.credential("b").ok().flatten().is_none());
    assert_eq!(harness.saved().servers.len(), 1);
    assert!(!harness.controller.queue().is_empty());

    harness
        .controller
        .handle(Command::RemoveServer("demo".into()));
    let saved = harness.saved();
    assert!(saved.servers.is_empty() && saved.current_server_id.is_none());
    assert!(harness.session.get().is_none());
    assert!(harness.controller.queue().is_empty());
    assert_eq!(harness.audio.take(), ["stop"]);
}

#[tokio::test]
async fn connecting_to_another_server_clears_the_queue() {
    let mut harness = harness(two_servers());
    harness.start();
    harness.controller.handle(Command::Connected {
        config: FakeServer::config_for_demo(),
        others: Vec::new(),
    });
    assert!(!harness.controller.queue().is_empty());
    harness.controller.handle(Command::Connected {
        config: server("b"),
        others: Vec::new(),
    });
    assert!(harness.controller.queue().is_empty());
    assert_eq!(harness.saved().current_server_id.as_deref(), Some("b"));
}

#[tokio::test]
async fn interface_and_integrations_are_saved_and_survive_shutdown() {
    let mut harness = harness(Settings::default());
    harness.controller.handle(Command::SaveInterface {
        queue_panel_open: true,
        follow_current_song: true,
    });
    let integrations = IntegrationSettings {
        discord_rich_presence: true,
        lyrics_sources: vec!["genius".into()],
        ..IntegrationSettings::default()
    };
    harness
        .controller
        .handle(Command::SaveIntegrations(integrations.clone()));
    let (reply, snapshot) = oneshot::channel();
    harness.controller.handle(Command::GetSettings(reply));
    let snapshot = snapshot.await.unwrap_or_else(|error| panic!("{error}"));
    assert!(snapshot.settings.ui.follow_current_song);
    assert_eq!(snapshot.devices.len(), 1);

    let (done, _finished) = oneshot::channel();
    harness.controller.handle(Command::Shutdown {
        ui: UiSettings::default(),
        done,
    });
    let saved = harness.saved();
    assert!(saved.ui.follow_current_song);
    assert!(!saved.ui.queue_panel_open);
    assert_eq!(saved.integrations, integrations);
}
