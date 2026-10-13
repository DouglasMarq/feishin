use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use feishin_core::model::{Song, SongListQuery};
use feishin_core::queue::{QueueView, Uid};
use feishin_player::PlayerEvent;
use feishin_storage::settings::{Settings, UiSettings};
use tokio::sync::{mpsc, oneshot};

use crate::audio::{AudioOutput, Track};
use crate::controller::{Command, Controller, Deps, UiSink, UiUpdate};
use crate::demo::{FakeServer, test_song};
use crate::list_stream::{ListRow, SongSource};
use crate::media::NoMediaSession;
use crate::session::Session;
use crate::store::{JsonStore, Store};

const COMMAND_WAIT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AudioCall {
    Load(Uid, u64),
    Preload(Uid),
    ClearNext,
    Play,
    Pause,
    Stop,
    Seek(u64),
}

#[derive(Clone, Default)]
pub(super) struct RecordingAudio(Arc<Mutex<Vec<AudioCall>>>);

impl RecordingAudio {
    pub(super) fn take(&self) -> Vec<AudioCall> {
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
pub(super) struct RecordingUi(Mutex<Vec<UiUpdate>>);

impl UiSink for RecordingUi {
    fn send(&self, update: UiUpdate) {
        if let Ok(mut updates) = self.0.lock() {
            updates.push(update);
        }
    }
}

impl RecordingUi {
    pub(super) fn last_queue(&self) -> Option<Vec<Uid>> {
        self.last_queue_view().map(|view| view.uids().collect())
    }

    pub(super) fn last_queue_view(&self) -> Option<QueueView> {
        let updates = self.0.lock().ok()?;
        updates.iter().rev().find_map(|update| match update {
            UiUpdate::Queue(view) => Some(view.clone()),
            _ => None,
        })
    }

    pub(super) fn errors(&self) -> Vec<String> {
        let updates = self.0.lock().unwrap();
        updates
            .iter()
            .filter_map(|update| match update {
                UiUpdate::Error(message) => Some(message.clone()),
                _ => None,
            })
            .collect()
    }

    pub(super) fn clear(&self) {
        self.0.lock().unwrap().clear();
    }

    pub(super) fn any(&self, predicate: impl Fn(&UiUpdate) -> bool) -> bool {
        self.0.lock().unwrap().iter().any(predicate)
    }
}

pub(super) struct Harness {
    pub(super) controller: Controller,
    pub(super) audio: RecordingAudio,
    pub(super) ui: Arc<RecordingUi>,
    pub(super) server: Arc<FakeServer>,
    pub(super) commands: mpsc::UnboundedReceiver<Command>,
    pub(super) store: Arc<JsonStore>,
    pub(super) temp_dir: tempfile::TempDir,
}

pub(super) fn start_harness_with(
    settings: Settings,
    song_count: usize,
    temp_dir: Option<tempfile::TempDir>,
) -> Harness {
    let temp_dir = temp_dir
        .or_else(|| tempfile::tempdir().ok())
        .unwrap_or_else(|| panic!("temp dir"));
    let store = Arc::new(JsonStore::new(temp_dir.path()));
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
        temp_dir,
    }
}

pub(super) fn start_harness(settings: Settings) -> Harness {
    start_harness_with(settings, 100, None)
}

pub(super) fn test_settings(gapless: bool, scrobble: bool) -> Settings {
    let mut settings = Settings::default();
    settings.playback.gapless = gapless;
    settings.playback.scrobble = scrobble;
    settings
}

pub(super) fn test_songs(count: usize, duration_ms: u64) -> Vec<Song> {
    (0..count)
        .map(|index| test_song(&format!("s{index}"), duration_ms))
        .collect()
}

pub(super) fn song_ids(songs: &[Song]) -> Vec<String> {
    songs.iter().map(|song| song.id.clone()).collect()
}

pub(super) async fn settle() {
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
}

impl Harness {
    pub(super) fn current_uid(&self) -> Option<Uid> {
        self.controller.queue().current().map(|item| item.uid)
    }

    pub(super) fn current_id(&self) -> Option<String> {
        self.controller
            .queue()
            .current()
            .map(|item| item.track.song_id.to_string())
    }

    pub(super) fn queued_ids(&self) -> Vec<String> {
        self.controller
            .queue()
            .display_order()
            .iter()
            .map(|item| item.track.song_id.to_string())
            .collect()
    }

    pub(super) fn uid_of(&self, song_id: &str) -> Option<Uid> {
        self.controller
            .queue()
            .display_order()
            .iter()
            .find(|item| item.track.song_id == song_id)
            .map(|item| item.uid)
    }

    pub(super) fn player(&mut self, event: PlayerEvent) {
        self.controller.handle(Command::Player(event));
    }

    pub(super) fn start_current(&mut self) -> Uid {
        let uid = self.current_uid().unwrap_or(Uid::MAX);
        self.player(PlayerEvent::Started { uid });
        uid
    }

    pub(super) fn request_from_list(&mut self, query: SongListQuery, row: usize, song_id: &str) {
        self.controller.handle(Command::PlayFromList {
            source: SongSource::Library(query),
            song_id: Some(song_id.to_owned()),
            row_index: ListRow(row),
        });
    }

    pub(super) async fn play_from_list(&mut self, query: SongListQuery, row: usize, song_id: &str) {
        self.request_from_list(query, row, song_id);
        self.finish_streams().await;
    }

    pub(super) async fn handle_next_command(&mut self) {
        let next = tokio::time::timeout(COMMAND_WAIT, self.commands.recv()).await;
        match next {
            Ok(Some(command)) => {
                self.controller.handle(command);
            }
            _ => panic!("no follow-up command arrived"),
        }
    }

    pub(super) async fn finish_streams(&mut self) {
        while self.controller.is_streaming() {
            self.handle_next_command().await;
        }
    }

    pub(super) fn connect(&mut self) {
        self.controller.handle(Command::Connected {
            config: FakeServer::config_for_demo(),
            others: Vec::new(),
        });
    }

    pub(super) fn shutdown_with_ui(&mut self, ui: UiSettings) -> oneshot::Receiver<()> {
        let (done, finished) = oneshot::channel();
        assert!(!self.controller.handle(Command::Shutdown { ui, done }));
        finished
    }

    pub(super) fn shutdown(&mut self) -> oneshot::Receiver<()> {
        self.shutdown_with_ui(UiSettings::default())
    }
}
