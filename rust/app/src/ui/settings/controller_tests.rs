use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use feishin_core::queue::Uid;
use feishin_player::{AudioDevice, PlayerEvent};
use feishin_storage::appearance::{AccentColor, AppearanceSettings, ThemeId};
use feishin_storage::settings::{
    IntegrationSettings, LyricsSourceSetting, PlaybackSettings, ReplayGainSetting, Settings,
    TraySettings, UiSettings,
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
struct RecordingAudio {
    calls: Arc<Mutex<Vec<String>>>,
}

impl RecordingAudio {
    fn log(&self, call: String) -> Result<()> {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(call);
        }
        Ok(())
    }

    fn take(&self) -> Vec<String> {
        self.calls
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
    fn set_muted(&self, _is_muted: bool) -> Result<()> {
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
    fn set_crossfade(&self, crossfade: Duration) -> Result<()> {
        self.log(format!("crossfade {}s", crossfade.as_secs()))
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
        covers: Arc::new(crate::covers::DemoCovers),
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
            volume_percent: 5,
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
    assert_eq!(
        saved.volume_percent,
        PlaybackSettings::default().volume_percent
    );

    harness
        .controller
        .handle(Command::ApplyPlayback(PlaybackSettings {
            is_gapless: false,
            ..saved
        }));
    assert_eq!(harness.audio.take(), ["clear"]);
}

#[tokio::test]
async fn crossfade_changes_reach_the_player_and_toggles_refresh_the_preload() {
    let mut harness = harness(Settings::default());
    harness.start();
    let next = harness
        .controller
        .queue()
        .peek_next()
        .map_or(Uid::MAX, |item| item.uid);
    let apply_crossfade = |harness: &mut Harness, crossfade_seconds| {
        let playback = PlaybackSettings {
            crossfade_seconds,
            ..harness.saved().playback
        };
        harness.controller.handle(Command::ApplyPlayback(playback));
        harness.audio.take()
    };

    assert_eq!(
        apply_crossfade(&mut harness, 4),
        [
            "crossfade 4s".to_owned(),
            "clear".into(),
            format!("preload {next}")
        ]
    );
    assert_eq!(harness.saved().playback.crossfade_seconds, 4);
    assert_eq!(apply_crossfade(&mut harness, 9), ["crossfade 9s"]);
    assert_eq!(
        apply_crossfade(&mut harness, 0),
        [
            "crossfade 0s".to_owned(),
            "clear".into(),
            format!("preload {next}")
        ]
    );
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
        credential: None,
        others: Vec::new(),
    });
    assert!(!harness.controller.queue().is_empty());
    harness.controller.handle(Command::Connected {
        config: server("b"),
        credential: None,
        others: Vec::new(),
    });
    assert!(harness.controller.queue().is_empty());
    assert_eq!(harness.saved().current_server_id.as_deref(), Some("b"));
}

#[tokio::test]
async fn interface_and_integrations_are_saved_and_survive_shutdown() {
    let mut harness = harness(Settings::default());
    harness.controller.handle(Command::SaveInterface {
        is_queue_panel_open: true,
        is_following_current_song: true,
    });
    harness
        .controller
        .handle(Command::SaveLanguage(Some("de".into())));
    let tray = TraySettings {
        is_enabled: false,
        is_close_to_tray: true,
    };
    harness.controller.handle(Command::SaveTray(tray));
    let integrations = IntegrationSettings {
        is_discord_rich_presence_enabled: true,
        lyrics_sources: vec![LyricsSourceSetting::Genius],
        ..IntegrationSettings::default()
    };
    harness
        .controller
        .handle(Command::SaveIntegrations(integrations.clone()));
    let (reply, snapshot) = oneshot::channel();
    harness.controller.handle(Command::GetSettings(reply));
    let snapshot = snapshot.await.unwrap_or_else(|error| panic!("{error}"));
    assert!(snapshot.settings.ui.is_following_current_song);
    assert_eq!(snapshot.devices.len(), 1);

    let (done, _finished) = oneshot::channel();
    harness.controller.handle(Command::Shutdown {
        ui: UiSettings::default(),
        done,
    });
    let saved = harness.saved();
    assert!(saved.ui.is_following_current_song);
    assert_eq!(saved.ui.language.as_deref(), Some("de"));
    assert_eq!(saved.ui.tray, tray);
    assert!(!saved.ui.is_queue_panel_open);
    assert_eq!(saved.integrations, integrations);
}

#[tokio::test]
async fn appearance_is_saved_and_survives_shutdown() {
    let mut harness = harness(Settings::default());
    let appearance = AppearanceSettings {
        theme: ThemeId::new("nord"),
        is_using_theme_accent: false,
        accent: AccentColor::new(0xaa, 0x6e, 0xd8),
        ..AppearanceSettings::default()
    };
    harness
        .controller
        .handle(Command::SaveAppearance(appearance.clone()));
    assert_eq!(harness.saved().appearance, appearance);
    let (done, _finished) = oneshot::channel();
    harness.controller.handle(Command::Shutdown {
        ui: UiSettings::default(),
        done,
    });
    assert_eq!(harness.saved().appearance, appearance);
}
