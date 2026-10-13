use std::sync::{Arc, Mutex};

use anyhow::Result;
use feishin_core::queue::Uid;
use feishin_platform::{MediaControlEvent, NowPlaying, PlaybackStatus};
use tokio::sync::mpsc;

use super::harness::{
    AudioCall, RecordingAudio, RecordingUi, start_harness, test_settings, test_songs,
};
use crate::controller::{Command, Controller, Deps};
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
    settings.playback.volume = 50;
    let deps = Deps {
        audio: Box::new(RecordingAudio::default()),
        media: Box::new(media.clone()),
        store: Arc::new(JsonStore::new(temp_dir.path())),
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
