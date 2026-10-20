use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use async_trait::async_trait;
use feishin_api::MusicServer;
use feishin_core::queue::Uid;
use feishin_platform::{MediaControlEvent, NowPlaying, PlaybackStatus};
use feishin_storage::cover_cache::DecodedImage;
use tokio::sync::mpsc;

use super::harness::{
    AudioCall, RecordingAudio, RecordingUi, start_harness, test_settings, test_songs,
};
use crate::controller::{Command, Controller, Deps};
use crate::covers::{CoverProvider, DemoCovers};
use crate::demo::FakeServer;
use crate::media::MediaSession;
use crate::session::Session;
use crate::store::JsonStore;

#[derive(Clone, Default)]
struct RecordingMedia(Arc<Mutex<Vec<f64>>>);

impl RecordingMedia {
    fn volumes(&self) -> Vec<f64> {
        self.0
            .lock()
            .map(|volumes| volumes.clone())
            .unwrap_or_default()
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
async fn media_keys_drive_playback() {
    let mut harness = start_harness(test_settings(false, false));
    harness.controller.play_now(test_songs(2, 100_000), "s0");
    harness.start_current();
    harness.audio.take();
    harness
        .controller
        .handle(Command::Media(MediaControlEvent::Toggle));
    assert_eq!(harness.audio.take(), [AudioCall::Pause]);
    harness
        .controller
        .handle(Command::Media(MediaControlEvent::Pause));
    assert_eq!(harness.audio.take(), []);
    harness
        .controller
        .handle(Command::Media(MediaControlEvent::Next));
    let second = harness.uid_of("s1").unwrap_or(Uid::MAX);
    assert_eq!(harness.audio.take(), [AudioCall::Load(second, 0)]);
}

#[tokio::test]
async fn media_session_follows_volume_and_mute() {
    let temp_dir = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir: {error}"));
    let media = RecordingMedia::default();
    let (commands, _receiver) = mpsc::unbounded_channel();
    let mut settings = test_settings(false, false);
    settings.playback.volume_percent = 50;
    let deps = Deps {
        audio: Box::new(RecordingAudio::default()),
        media: Box::new(media.clone()),
        store: Arc::new(JsonStore::new(temp_dir.path())),
        session: Session::default(),
        ui: Arc::new(RecordingUi::default()),
        commands,
        covers: Arc::new(crate::covers::DemoCovers),
    };
    let mut controller = Controller::new(deps, settings, None, Some(1));
    controller.handle(Command::SetVolume(40));
    controller.handle(Command::ToggleMute);
    controller.handle(Command::Media(MediaControlEvent::SetVolume(0.25)));
    assert_eq!(media.volumes(), [0.5, 0.4, 0.0, 0.25]);
}

#[derive(Clone, Default)]
struct RecordingMetadata(Arc<Mutex<Vec<NowPlaying>>>);

impl RecordingMetadata {
    fn cover_files(&self) -> Vec<Option<PathBuf>> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .map(|now_playing| now_playing.cover_file.clone())
            .collect()
    }
}

impl MediaSession for RecordingMetadata {
    fn set_metadata(&mut self, now_playing: &NowPlaying) -> Result<()> {
        self.0.lock().unwrap().push(now_playing.clone());
        Ok(())
    }
    fn set_status(&mut self, _status: PlaybackStatus) -> Result<()> {
        Ok(())
    }
    fn set_volume(&mut self, _volume: f64) -> Result<()> {
        Ok(())
    }
}

struct DiskCovers {
    file: PathBuf,
    is_cached: AtomicBool,
}

#[async_trait]
impl CoverProvider for DiskCovers {
    async fn cover(
        &self,
        server: &dyn MusicServer,
        cover_art_id: &str,
        size: u32,
    ) -> Result<Arc<DecodedImage>> {
        self.is_cached.store(true, Ordering::SeqCst);
        DemoCovers.cover(server, cover_art_id, size).await
    }

    async fn cover_from_url(
        &self,
        cache_key: &str,
        url: &str,
        size: u32,
    ) -> Result<Arc<DecodedImage>> {
        DemoCovers.cover_from_url(cache_key, url, size).await
    }

    fn cover_file(
        &self,
        _server: &dyn MusicServer,
        _cover_art_id: &str,
        _size: u32,
    ) -> Option<PathBuf> {
        self.is_cached
            .load(Ordering::SeqCst)
            .then(|| self.file.clone())
    }
}

#[tokio::test]
async fn given_a_server_cover_the_media_session_gets_its_cached_local_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let cover_file = temp_dir.path().join("cover.jpg");
    let media = RecordingMetadata::default();
    let (commands, mut receiver) = mpsc::unbounded_channel();
    let session = Session::default();
    session.set(Some(Arc::new(FakeServer::new(3))));
    let deps = Deps {
        audio: Box::new(RecordingAudio::default()),
        media: Box::new(media.clone()),
        store: Arc::new(JsonStore::new(temp_dir.path())),
        session,
        ui: Arc::new(RecordingUi::default()),
        commands,
        covers: Arc::new(DiskCovers {
            file: cover_file.clone(),
            is_cached: AtomicBool::new(false),
        }),
    };
    let mut controller = Controller::new(deps, test_settings(false, false), None, Some(1));
    let mut songs = test_songs(2, 100_000);
    songs[0].cover_art_id = Some("cover-1".into());
    controller.play_now(songs, "s0");
    let uid = controller.queue().current().map(|item| item.uid).unwrap();
    controller.handle(Command::Player(feishin_player::PlayerEvent::Started {
        uid,
    }));
    assert_eq!(media.cover_files(), [None]);

    loop {
        let command = receiver.recv().await.unwrap();
        let is_cover = matches!(command, Command::MediaCoverCached { .. });
        controller.handle(command);
        if is_cover {
            break;
        }
    }

    assert_eq!(media.cover_files(), [None, Some(cover_file)]);
}
