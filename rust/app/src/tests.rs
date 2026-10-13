//! Controller tests driven by the fake server and a recording audio output.

mod integrations;

use std::sync::{Arc, Mutex};

use anyhow::Result;
use feishin_core::model::{Paging, Song, SongListQuery, SongSort};
use feishin_core::queue::{RepeatMode, Uid};
use feishin_platform::MediaControlEvent;
use feishin_player::PlayerEvent;
use feishin_storage::settings::{ColumnSetting, Settings, UiSettings};
use tokio::sync::{mpsc, oneshot};

use crate::audio::{AudioOutput, Track};
use crate::controller::{Command, Controller, Deps, UiSink, UiUpdate};
use crate::demo::{FakeServer, ScrobbleCall, test_song};
use crate::media::NoMediaSession;
use crate::session::Session;
use crate::store::{JsonStore, Store};

#[derive(Debug, Clone, PartialEq, Eq)]
enum AudioCall {
    Load(Uid, u64),
    Preload(Uid),
    ClearNext,
    Play,
    Pause,
    Stop,
    Seek(u64),
}

#[derive(Clone, Default)]
struct RecordingAudio(Arc<Mutex<Vec<AudioCall>>>);

impl RecordingAudio {
    fn take(&self) -> Vec<AudioCall> {
        self.0
            .lock()
            .map(|mut calls| calls.drain(..).collect())
            .unwrap_or_default()
    }

    fn record(&self, call: AudioCall) -> Result<()> {
        if let Ok(mut calls) = self.0.lock() {
            calls.push(call);
        }
        Ok(())
    }
}

impl AudioOutput for RecordingAudio {
    fn load(&self, track: &Track, start_ms: u64) -> Result<()> {
        self.record(AudioCall::Load(track.uid, start_ms))
    }
    fn preload_next(&self, track: &Track) -> Result<()> {
        self.record(AudioCall::Preload(track.uid))
    }
    fn clear_next(&self) -> Result<()> {
        self.record(AudioCall::ClearNext)
    }
    fn play(&self) -> Result<()> {
        self.record(AudioCall::Play)
    }
    fn pause(&self) -> Result<()> {
        self.record(AudioCall::Pause)
    }
    fn stop(&self) -> Result<()> {
        self.record(AudioCall::Stop)
    }
    fn seek(&self, position_ms: u64) -> Result<()> {
        self.record(AudioCall::Seek(position_ms))
    }
    fn set_volume(&self, _volume: u8) -> Result<()> {
        Ok(())
    }
    fn set_muted(&self, _muted: bool) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct RecordingUi(Mutex<Vec<UiUpdate>>);

impl UiSink for RecordingUi {
    fn send(&self, update: UiUpdate) {
        if let Ok(mut updates) = self.0.lock() {
            updates.push(update);
        }
    }
}

impl RecordingUi {
    fn last_queue(&self) -> Option<Vec<Uid>> {
        let updates = self.0.lock().ok()?;
        updates.iter().rev().find_map(|update| match update {
            UiUpdate::Queue { entries, .. } => Some(entries.iter().map(|e| e.uid).collect()),
            _ => None,
        })
    }
}

struct Harness {
    controller: Controller,
    audio: RecordingAudio,
    ui: Arc<RecordingUi>,
    server: Arc<FakeServer>,
    commands: mpsc::UnboundedReceiver<Command>,
    store: Arc<JsonStore>,
    _dir: tempfile::TempDir,
}

fn harness_with(settings: Settings, song_count: usize, dir: Option<tempfile::TempDir>) -> Harness {
    let dir = dir
        .or_else(|| tempfile::tempdir().ok())
        .unwrap_or_else(|| panic!("temp dir"));
    let store = Arc::new(JsonStore::new(dir.path()));
    let stored = store.load_queue().ok().flatten();
    let server = Arc::new(FakeServer::new(song_count));
    let session = Session::default();
    session.set(Some(server.clone()));
    let audio = RecordingAudio::default();
    let ui = Arc::new(RecordingUi::default());
    let (sender, commands) = mpsc::unbounded_channel();
    let deps = Deps {
        audio: Box::new(audio.clone()),
        media: Box::new(NoMediaSession),
        store: store.clone(),
        session,
        ui: ui.clone(),
        commands: sender,
    };
    Harness {
        controller: Controller::new(deps, settings, stored, Some(42)),
        audio,
        ui,
        server,
        commands,
        store,
        _dir: dir,
    }
}

fn harness(settings: Settings) -> Harness {
    harness_with(settings, 100, None)
}

fn settings(gapless: bool, scrobble: bool) -> Settings {
    let mut settings = Settings::default();
    settings.playback.gapless = gapless;
    settings.playback.scrobble = scrobble;
    settings
}

fn songs(n: usize, duration_ms: u64) -> Vec<Song> {
    (0..n)
        .map(|i| test_song(&format!("s{i}"), duration_ms))
        .collect()
}

impl Harness {
    fn current_uid(&self) -> Option<Uid> {
        self.controller.queue().current().map(|item| item.uid)
    }

    fn current_id(&self) -> Option<String> {
        self.controller
            .queue()
            .current()
            .map(|item| item.song.id.clone())
    }

    fn uid_of(&self, song_id: &str) -> Option<Uid> {
        self.controller
            .queue()
            .display_order()
            .iter()
            .find(|item| item.song.id == song_id)
            .map(|item| item.uid)
    }

    fn player(&mut self, event: PlayerEvent) {
        self.controller.handle(Command::Player(event));
    }

    fn start_current(&mut self) -> Uid {
        let uid = self.current_uid().unwrap_or(Uid::MAX);
        self.player(PlayerEvent::Started { uid });
        uid
    }

    async fn play_from_list(&mut self, query: SongListQuery, song_id: &str) {
        self.controller.handle(Command::PlayFromList {
            query,
            song_id: song_id.to_owned(),
        });
        let follow_up = self.commands.recv().await;
        assert!(matches!(follow_up, Some(Command::PlayNow { .. })));
        if let Some(command) = follow_up {
            self.controller.handle(command);
        }
    }
}

async fn settle() {
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
}

#[tokio::test]
async fn play_from_list_queues_the_whole_list_and_loads_the_clicked_song() {
    let mut h = harness_with(settings(false, false), 1234, None);
    let query = SongListQuery {
        sort: SongSort::Name,
        ..SongListQuery::default()
    };
    let paging = Paging {
        start_index: 0,
        limit: 2000,
    };
    let expected: Vec<String> =
        feishin_api::MusicServer::song_list(h.server.as_ref(), &query, paging)
            .await
            .map(|page| page.items.into_iter().map(|s| s.id).collect())
            .unwrap_or_default();
    let clicked = expected[700].clone();

    h.play_from_list(query.clone(), &clicked).await;

    let queued: Vec<String> = h
        .controller
        .queue()
        .display_order()
        .iter()
        .map(|item| item.song.id.clone())
        .collect();
    assert_eq!(queued, expected);
    assert_eq!(h.current_id(), Some(clicked.clone()));
    let uid = h.current_uid().unwrap_or(Uid::MAX);
    assert_eq!(h.audio.take(), [AudioCall::Load(uid, 0)]);

    h.start_current();
    h.player(PlayerEvent::Position {
        position_ms: 30_000,
        duration_ms: None,
    });
    h.play_from_list(query, &clicked).await;
    let restarted = h.current_uid().unwrap_or(Uid::MAX);
    assert_eq!(h.current_id(), Some(clicked));
    assert_eq!(h.audio.take(), [AudioCall::Load(restarted, 0)]);
}

#[tokio::test]
async fn stale_list_fetches_are_ignored() {
    let mut h = harness(settings(false, false));
    let query = SongListQuery::default();
    h.controller.handle(Command::PlayFromList {
        query: query.clone(),
        song_id: "song-1".into(),
    });
    h.controller.handle(Command::PlayFromList {
        query,
        song_id: "song-2".into(),
    });
    for _ in 0..2 {
        if let Some(command) = h.commands.recv().await {
            h.controller.handle(command);
        }
    }
    assert_eq!(h.current_id().as_deref(), Some("song-2"));
    assert_eq!(h.audio.take().len(), 1);
}

#[tokio::test]
async fn track_end_advances_and_stops_after_the_last_song() {
    let mut h = harness(settings(false, false));
    h.controller.play_now(songs(2, 1000), "s0");
    let first = h.start_current();
    h.audio.take();

    h.player(PlayerEvent::TrackEnded { uid: first });
    let second = h.uid_of("s1").unwrap_or(Uid::MAX);
    assert_eq!(h.audio.take(), [AudioCall::Load(second, 0)]);

    h.start_current();
    h.player(PlayerEvent::TrackEnded { uid: second });
    assert_eq!(h.audio.take(), [AudioCall::Stop]);
    assert!(!h.controller.is_playing());
}

#[tokio::test]
async fn gapless_preloads_on_start_and_follows_the_takeover() {
    let mut h = harness(settings(true, false));
    h.controller.play_now(songs(3, 1000), "s0");
    let first = h.start_current();
    let second = h.uid_of("s1").unwrap_or(Uid::MAX);
    let third = h.uid_of("s2").unwrap_or(Uid::MAX);
    assert_eq!(
        h.audio.take(),
        [AudioCall::Load(first, 0), AudioCall::Preload(second)]
    );

    h.player(PlayerEvent::TrackEnded { uid: first });
    assert_eq!(h.audio.take(), []);
    h.player(PlayerEvent::Started { uid: second });
    assert_eq!(h.current_uid(), Some(second));
    assert_eq!(h.audio.take(), [AudioCall::Preload(third)]);

    h.controller.handle(Command::Remove(vec![third]));
    assert_eq!(h.audio.take(), [AudioCall::ClearNext]);
}

#[tokio::test]
async fn repeat_one_preloads_the_same_song() {
    let mut h = harness(settings(true, false));
    h.controller.play_now(songs(2, 1000), "s0");
    h.controller.handle(Command::CycleRepeat);
    h.controller.handle(Command::CycleRepeat);
    assert_eq!(h.controller.queue().repeat(), RepeatMode::One);
    let first = h.start_current();
    assert_eq!(h.audio.take().last(), Some(&AudioCall::Preload(first)));
    h.player(PlayerEvent::Started { uid: first });
    assert_eq!(h.current_uid(), Some(first));
}

#[tokio::test]
async fn scrobbles_now_playing_on_start_and_submits_at_half() {
    let mut h = harness(settings(false, true));
    h.controller.play_now(songs(1, 60_000), "s0");
    h.start_current();
    settle().await;
    assert_eq!(
        h.server.scrobbles(),
        [ScrobbleCall {
            song_id: "s0".into(),
            submission: false,
            timestamp_ms: None
        }]
    );

    for tick in 0..=(29_000 / 250) {
        h.player(PlayerEvent::Position {
            position_ms: tick * 250,
            duration_ms: Some(60_000),
        });
    }
    settle().await;
    assert_eq!(h.server.scrobbles().len(), 1);

    for tick in (29_000 / 250)..=(40_000 / 250) {
        h.player(PlayerEvent::Position {
            position_ms: tick * 250,
            duration_ms: Some(60_000),
        });
    }
    settle().await;
    let calls = h.server.scrobbles();
    assert_eq!(calls.len(), 2);
    assert!(calls[1].submission && calls[1].song_id == "s0");
}

#[tokio::test]
async fn scrobbling_disabled_sends_nothing() {
    let mut h = harness(settings(false, false));
    h.controller.play_now(songs(1, 10_000), "s0");
    h.start_current();
    for tick in 0..40 {
        h.player(PlayerEvent::Position {
            position_ms: tick * 250,
            duration_ms: Some(10_000),
        });
    }
    settle().await;
    assert!(h.server.scrobbles().is_empty());
}

#[tokio::test]
async fn previous_restarts_after_ten_seconds() {
    let mut h = harness(settings(false, false));
    h.controller.play_now(songs(2, 100_000), "s1");
    h.start_current();
    h.player(PlayerEvent::Position {
        position_ms: 12_000,
        duration_ms: None,
    });
    h.audio.take();
    h.controller.handle(Command::Previous);
    assert_eq!(h.audio.take(), [AudioCall::Seek(0)]);
    h.controller.handle(Command::Previous);
    let first = h.uid_of("s0").unwrap_or(Uid::MAX);
    assert_eq!(h.audio.take(), [AudioCall::Load(first, 0)]);
}

#[tokio::test]
async fn queue_panel_reorders_and_shows_play_order() {
    let mut h = harness(settings(false, false));
    h.controller.play_now(songs(3, 1000), "s0");
    let uids: Vec<Uid> = ["s0", "s1", "s2"]
        .iter()
        .map(|id| h.uid_of(id).unwrap_or(Uid::MAX))
        .collect();
    h.controller.handle(Command::Move {
        uids: vec![uids[2]],
        before: Some(uids[0]),
    });
    assert_eq!(h.ui.last_queue(), Some(vec![uids[2], uids[0], uids[1]]));

    h.controller.handle(Command::ToggleShuffle);
    let shown = h.ui.last_queue().unwrap_or_default();
    let order: Vec<Uid> = h
        .controller
        .queue()
        .display_order()
        .iter()
        .map(|i| i.uid)
        .collect();
    assert_eq!(shown, order);
    assert_eq!(shown.first(), h.current_uid().as_ref());
}

#[tokio::test]
async fn settings_and_queue_survive_a_restart() {
    let mut h = harness(settings(false, false));
    h.controller.play_now(songs(5, 100_000), "s3");
    h.start_current();
    h.controller.handle(Command::ToggleShuffle);
    h.controller.handle(Command::CycleRepeat);
    h.controller.handle(Command::SetVolume(33));
    h.player(PlayerEvent::Position {
        position_ms: 12_345,
        duration_ms: None,
    });
    let snapshot = h.controller.queue().snapshot();
    let ui = UiSettings {
        track_columns: vec![ColumnSetting {
            id: "title".into(),
            width: 411.0,
            visible: true,
        }],
        queue_panel_open: true,
        ..UiSettings::default()
    };
    let (done, finished) = oneshot::channel();
    assert!(!h.controller.handle(Command::Shutdown {
        ui: ui.clone(),
        done
    }));
    assert!(finished.await.is_ok());

    let saved = h.store.load_settings().unwrap_or_default();
    assert_eq!(saved.playback.volume, 33);
    assert_eq!(saved.ui, ui);

    let mut restored = harness_with(saved, 100, Some(h._dir));
    assert_eq!(restored.controller.queue().snapshot(), snapshot);
    assert!(restored.controller.queue().shuffle());
    assert_eq!(restored.controller.queue().repeat(), RepeatMode::All);
    assert_eq!(restored.current_id().as_deref(), Some("s3"));
    assert!(restored.audio.take().is_empty());

    restored.controller.handle(Command::TogglePause);
    let uid = restored.current_uid().unwrap_or(Uid::MAX);
    assert_eq!(restored.audio.take(), [AudioCall::Load(uid, 12_345)]);
}

#[tokio::test]
async fn media_keys_drive_playback() {
    let mut h = harness(settings(false, false));
    h.controller.play_now(songs(2, 100_000), "s0");
    h.start_current();
    h.audio.take();
    h.controller
        .handle(Command::Media(MediaControlEvent::Toggle));
    assert_eq!(h.audio.take(), [AudioCall::Pause]);
    h.controller
        .handle(Command::Media(MediaControlEvent::Pause));
    assert_eq!(h.audio.take(), []);
    h.controller.handle(Command::Media(MediaControlEvent::Next));
    let second = h.uid_of("s1").unwrap_or(Uid::MAX);
    assert_eq!(h.audio.take(), [AudioCall::Load(second, 0)]);
}
