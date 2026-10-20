use std::sync::{Arc, Mutex};

use anyhow::Result;
use feishin_core::queue::QueueTrack;
use feishin_platform::{NowPlaying, PlaybackStatus};
use feishin_youtube::CookieBrowser;
use tokio::sync::mpsc;

use super::harness::{RecordingAudio, RecordingUi, settle, song_ids, test_songs};
use super::youtube_support::{FUTURE_URL, FakeYtDlp, play_tracks, youtube_track};
use crate::controller::{Command, Controller, Deps, YouTubeCommand};
use crate::media::MediaSession;
use crate::session::Session;
use crate::store::{JsonStore, Store};

#[derive(Clone, Default)]
struct RecordingMetadata(Arc<Mutex<Vec<NowPlaying>>>);

impl MediaSession for RecordingMetadata {
    fn set_metadata(&mut self, now_playing: &NowPlaying) -> Result<()> {
        if let Ok(mut recorded) = self.0.lock() {
            recorded.push(now_playing.clone());
        }
        Ok(())
    }
    fn set_status(&mut self, _status: PlaybackStatus) -> Result<()> {
        Ok(())
    }
    fn set_volume(&mut self, _volume: f64) -> Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn youtube_tracks_are_never_scrobbled() {
    let fake = FakeYtDlp::printing(FUTURE_URL).await;
    let mut harness = fake.harness(false, true);
    play_tracks(&mut harness, vec![youtube_track("abc")]);
    harness.handle_next_command().await;
    harness.start_current();
    settle().await;
    assert!(harness.server.scrobbles().is_empty());
}

#[tokio::test]
async fn youtube_tracks_stay_out_of_the_server_play_queue() {
    let fake = FakeYtDlp::printing(FUTURE_URL).await;
    let mut harness = fake.harness(false, false);
    let songs: Vec<QueueTrack> = test_songs(2, 100_000)
        .iter()
        .map(QueueTrack::from)
        .collect();
    let tracks = vec![songs[0].clone(), youtube_track("abc"), songs[1].clone()];
    play_tracks(&mut harness, tracks);
    harness.start_current();
    settle().await;
    let saved = harness.server.saved_play_queue().1.unwrap();
    assert_eq!(song_ids(&saved.songs), ["s0", "s1"]);
    assert_eq!(saved.current_song_id.as_deref(), Some("s0"));

    harness.controller.handle(Command::Next);
    harness.handle_next_command().await;
    settle().await;
    let saved = harness.server.saved_play_queue().1.unwrap();
    assert_eq!(song_ids(&saved.songs), ["s0", "s1"]);
    assert_eq!(saved.current_song_id, None);
}

#[tokio::test]
async fn media_sessions_show_the_youtube_thumbnail() {
    let fake = FakeYtDlp::printing(FUTURE_URL).await;
    let temp_dir = tempfile::tempdir().unwrap();
    let media = RecordingMetadata::default();
    let (commands, mut receiver) = mpsc::unbounded_channel();
    let audio = RecordingAudio::default();
    let deps = Deps {
        audio: Box::new(audio.clone()),
        media: Box::new(media.clone()),
        store: Arc::new(JsonStore::new(temp_dir.path())),
        session: Session::default(),
        ui: Arc::new(RecordingUi::default()),
        commands,
        covers: Arc::new(crate::covers::DemoCovers),
    };
    let mut controller = Controller::new(deps, fake.settings(false, false), None, Some(1));
    controller.handle(Command::Tracks(crate::controller::TrackCommand::PlayNow {
        tracks: vec![youtube_track("abc")],
        start_index: None,
    }));
    controller.handle(receiver.recv().await.unwrap());
    let uid = controller.queue().current().map(|item| item.uid).unwrap();
    controller.handle(Command::Player(feishin_player::PlayerEvent::Started {
        uid,
    }));
    let recorded = media.0.lock().unwrap().clone();
    let now_playing = recorded.last().unwrap();
    assert_eq!(now_playing.track_id, "yt:abc");
    assert_eq!(now_playing.artists, ["Singer"]);
    assert_eq!(
        now_playing.cover_url.as_deref(),
        Some("https://i.ytimg.com/vi/abc/hqdefault.jpg")
    );
}

#[tokio::test]
async fn youtube_settings_are_saved() {
    let fake = FakeYtDlp::printing(FUTURE_URL).await;
    let mut harness = fake.harness(false, false);
    let mut youtube = harness.store.load_settings().unwrap().youtube;
    youtube.cookies_from_browser = Some(CookieBrowser::Chromium);
    harness
        .controller
        .handle(Command::YouTube(YouTubeCommand::SaveSettings(
            youtube.clone(),
        )));
    assert_eq!(harness.store.load_settings().unwrap().youtube, youtube);
}
