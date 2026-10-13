//! Play queue sync, sleep timer, previous, credential persistence and media session volume.

use std::sync::{Arc, Mutex};

use anyhow::Result;
use feishin_api::{Credential, MusicServer};
use feishin_core::model::SongListQuery;
use feishin_core::queue::Uid;
use feishin_platform::{MediaControlEvent, NowPlaying, PlaybackStatus};
use feishin_player::PlayerEvent;
use feishin_storage::settings::UiSettings;
use tokio::sync::{mpsc, oneshot};

use super::{AudioCall, Harness, RecordingAudio, RecordingUi, harness, settings, settle, songs};
use crate::controller::{Command, Controller, Deps};
use crate::demo::FakeServer;
use crate::media::MediaSession;
use crate::session::Session;
use crate::sleep_timer::{SleepMode, SleepStatus};
use crate::store::{JsonStore, Store};

fn ids(songs: &[feishin_core::model::Song]) -> Vec<String> {
    songs.iter().map(|song| song.id.clone()).collect()
}

impl Harness {
    fn connect(&mut self) {
        self.controller.handle(Command::Connected {
            config: FakeServer::config_for_demo(),
            others: Vec::new(),
        });
    }

    fn shutdown(&mut self) -> oneshot::Receiver<()> {
        let (done, finished) = oneshot::channel();
        assert!(!self.controller.handle(Command::Shutdown {
            ui: UiSettings::default(),
            done,
        }));
        finished
    }
}

#[tokio::test]
async fn queue_changes_are_saved_to_the_server_once_debounced() {
    let mut h = harness(settings(false, false));
    h.controller.play_now(songs(4, 100_000), "s1");
    let first = h.start_current();
    h.controller.handle(Command::Remove(vec![first]));
    h.controller.handle(Command::TogglePause);
    settle().await;

    let (saves, saved) = h.server.saved_play_queue();
    assert_eq!(saves, 1);
    let saved = saved.unwrap_or_else(|| panic!("queue saved"));
    assert_eq!(ids(&saved.songs), ["s0", "s2", "s3"]);
    assert_eq!(saved.current_song_id.as_deref(), Some("s2"));

    h.controller.handle(Command::Seek(42_000));
    settle().await;
    let (saves, saved) = h.server.saved_play_queue();
    assert_eq!(saves, 2);
    assert_eq!(saved.map(|q| q.position_ms), Some(42_000));

    h.controller.handle(Command::Seek(42_000));
    settle().await;
    assert_eq!(h.server.saved_play_queue().0, 2);
}

#[tokio::test]
async fn shutdown_saves_the_queue_with_the_position() {
    let mut h = harness(settings(false, false));
    h.controller.play_now(songs(3, 100_000), "s2");
    h.start_current();
    h.player(PlayerEvent::Position {
        position_ms: 12_345,
        duration_ms: None,
    });
    assert!(h.shutdown().await.is_ok());
    let saved = h.server.saved_play_queue().1;
    assert_eq!(
        saved.map(|q| (q.current_song_id, q.position_ms)),
        Some((Some("s2".to_owned()), 12_345))
    );
}

#[tokio::test]
async fn empty_queue_is_restored_from_the_server_without_playing() {
    let mut h = harness(settings(false, false));
    let saved: Vec<String> = ["song-3", "song-4", "song-5"].map(String::from).into();
    let seeded = h
        .server
        .save_play_queue(&saved, Some("song-4"), 5_000)
        .await;
    assert!(seeded.is_ok());

    h.connect();
    let command = h.commands.recv().await;
    assert!(matches!(command, Some(Command::RestoreQueue { .. })));
    if let Some(command) = command {
        h.controller.handle(command);
    }
    let queued: Vec<String> = h
        .controller
        .queue()
        .display_order()
        .iter()
        .map(|item| item.song.id.clone())
        .collect();
    assert_eq!(queued, saved);
    assert_eq!(h.current_id().as_deref(), Some("song-4"));
    assert!(!h.controller.is_playing());
    assert!(h.audio.take().is_empty());

    h.controller.handle(Command::TogglePause);
    let uid = h.current_uid().unwrap_or(Uid::MAX);
    assert_eq!(h.audio.take(), [AudioCall::Load(uid, 5_000)]);
}

#[tokio::test]
async fn server_queue_does_not_replace_a_local_queue_or_a_newer_play() {
    let mut h = harness(settings(false, false));
    let seeded = h
        .server
        .save_play_queue(&["song-1".to_owned()], None, 0)
        .await;
    assert!(seeded.is_ok());

    h.connect();
    h.controller.handle(Command::PlayFromList {
        query: SongListQuery::default(),
        song_id: "song-2".into(),
    });
    for _ in 0..2 {
        if let Some(command) = h.commands.recv().await {
            h.controller.handle(command);
        }
    }
    assert_eq!(h.current_id().as_deref(), Some("song-2"));
    assert!(h.controller.queue().len() > 1);

    h.connect();
    settle().await;
    assert!(h.commands.try_recv().is_err());
}

#[tokio::test]
async fn timed_sleep_timer_counts_listening_time_and_pauses() {
    let mut h = harness(settings(false, false));
    h.controller.play_now(songs(2, 3_600_000), "s0");
    h.start_current();
    h.controller
        .handle(Command::SleepTimer(Some(SleepMode::Minutes(15))));
    h.audio.take();

    h.controller.handle(Command::SleepTick {
        elapsed_ms: 14 * 60_000,
    });
    h.controller.handle(Command::TogglePause);
    h.controller.handle(Command::SleepTick {
        elapsed_ms: 5 * 60_000,
    });
    assert_eq!(
        h.controller.sleep_status(),
        Some(SleepStatus::Remaining { ms: 60_000 })
    );
    h.controller.handle(Command::TogglePause);
    assert_eq!(h.audio.take(), [AudioCall::Pause, AudioCall::Play]);

    h.controller
        .handle(Command::SleepTick { elapsed_ms: 60_000 });
    assert_eq!(h.audio.take(), [AudioCall::Pause]);
    assert!(!h.controller.is_playing());
    assert_eq!(h.controller.sleep_status(), None);
}

#[tokio::test]
async fn end_of_track_sleep_timer_stops_on_the_next_song() {
    let mut h = harness(settings(true, false));
    h.controller.play_now(songs(3, 1000), "s0");
    let first = h.start_current();
    let second = h.uid_of("s1").unwrap_or(Uid::MAX);
    assert_eq!(h.audio.take().last(), Some(&AudioCall::Preload(second)));

    h.controller
        .handle(Command::SleepTimer(Some(SleepMode::EndOfTrack)));
    assert_eq!(h.audio.take(), [AudioCall::ClearNext]);

    h.player(PlayerEvent::TrackEnded { uid: first });
    assert_eq!(h.audio.take(), [AudioCall::Stop]);
    assert!(!h.controller.is_playing());
    assert_eq!(h.current_uid(), Some(second));
    assert_eq!(h.controller.sleep_status(), None);

    h.controller.handle(Command::TogglePause);
    assert_eq!(h.audio.take(), [AudioCall::Load(second, 0)]);
}

#[tokio::test]
async fn cancelling_end_of_track_restores_the_preload() {
    let mut h = harness(settings(true, false));
    h.controller.play_now(songs(2, 1000), "s0");
    h.start_current();
    h.controller
        .handle(Command::SleepTimer(Some(SleepMode::EndOfTrack)));
    h.audio.take();
    h.controller.handle(Command::SleepTimer(None));
    let second = h.uid_of("s1").unwrap_or(Uid::MAX);
    assert_eq!(h.audio.take(), [AudioCall::Preload(second)]);
}

#[tokio::test]
async fn previous_goes_back_early_in_a_track_and_restarts_the_first_one() {
    let mut h = harness(settings(false, false));
    h.controller.play_now(songs(2, 100_000), "s1");
    h.start_current();
    h.player(PlayerEvent::Position {
        position_ms: 9_000,
        duration_ms: None,
    });
    h.audio.take();
    h.controller.handle(Command::Previous);
    let first = h.uid_of("s0").unwrap_or(Uid::MAX);
    assert_eq!(h.audio.take(), [AudioCall::Load(first, 0)]);

    h.start_current();
    h.controller.handle(Command::Previous);
    assert_eq!(h.audio.take(), [AudioCall::Seek(0)]);
}

#[tokio::test]
async fn refreshed_credential_is_persisted_on_connect_and_shutdown() {
    let mut h = harness(settings(false, false));
    let credential = |token: &str| Credential::Navidrome {
        subsonic: "u=demo&s=a&t=b".into(),
        nd_token: Some(token.into()),
    };
    h.server.set_credential(credential("first"));
    h.connect();
    assert_eq!(
        h.store.credential("demo").ok().flatten(),
        Some(credential("first"))
    );

    h.server.set_credential(credential("refreshed"));
    assert!(h.shutdown().await.is_ok());
    assert_eq!(
        h.store.credential("demo").ok().flatten(),
        Some(credential("refreshed"))
    );
}

#[derive(Clone, Default)]
struct RecordingMedia(Arc<Mutex<Vec<f64>>>);

impl RecordingMedia {
    fn volumes(&self) -> Vec<f64> {
        self.0.lock().map(|v| v.clone()).unwrap_or_default()
    }
}

impl MediaSession for RecordingMedia {
    fn set_metadata(&mut self, _now_playing: &NowPlaying) -> Result<()> {
        Ok(())
    }
    fn set_status(&mut self, _status: PlaybackStatus) -> Result<()> {
        Ok(())
    }
    fn set_volume(&mut self, volume: f64) -> Result<()> {
        if let Ok(mut volumes) = self.0.lock() {
            volumes.push(volume);
        }
        Ok(())
    }
}

#[tokio::test]
async fn media_session_follows_volume_and_mute() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("temp dir: {e}"));
    let media = RecordingMedia::default();
    let (commands, _receiver) = mpsc::unbounded_channel();
    let mut settings = settings(false, false);
    settings.playback.volume = 50;
    let deps = Deps {
        audio: Box::new(RecordingAudio::default()),
        media: Box::new(media.clone()),
        store: Arc::new(JsonStore::new(dir.path())),
        session: Session::default(),
        ui: Arc::new(RecordingUi::default()),
        commands,
    };
    let mut controller = Controller::new(deps, settings, None, Some(1));
    controller.handle(Command::SetVolume(40));
    controller.handle(Command::ToggleMute);
    controller.handle(Command::Media(MediaControlEvent::SetVolume(0.25)));
    assert_eq!(media.volumes(), [0.5, 0.4, 0.0, 0.25]);
}
