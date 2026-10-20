use std::sync::{Arc, Mutex};

use anyhow::Result;
use feishin_core::model::RadioStation;
use feishin_platform::{NowPlaying, PlaybackStatus};
use feishin_player::PlayerEvent;
use tokio::sync::mpsc;

use super::harness::{RecordingAudio, RecordingUi, test_settings, test_songs};
use crate::controller::{Command, Controller, Deps, RADIO_STREAM_UID, RadioCommand};
use crate::media::MediaSession;
use crate::session::Session;
use crate::store::JsonStore;

#[derive(Debug, Clone, PartialEq)]
enum MediaCall {
    Title(String),
    Live(bool),
}

#[derive(Clone, Default)]
struct RecordingMedia(Arc<Mutex<Vec<MediaCall>>>);

impl RecordingMedia {
    fn record(&self, call: MediaCall) {
        if let Ok(mut calls) = self.0.lock() {
            calls.push(call);
        }
    }

    fn take(&self) -> Vec<MediaCall> {
        self.0
            .lock()
            .map(|mut calls| calls.drain(..).collect())
            .unwrap_or_default()
    }
}

impl MediaSession for RecordingMedia {
    fn set_metadata(&mut self, now_playing: &NowPlaying) -> Result<()> {
        self.record(MediaCall::Title(now_playing.title.clone()));
        Ok(())
    }
    fn set_status(&mut self, _status: PlaybackStatus) -> Result<()> {
        Ok(())
    }
    fn set_volume(&mut self, _volume: f64) -> Result<()> {
        Ok(())
    }
    fn set_live_stream(&mut self, live: bool) -> Result<()> {
        self.record(MediaCall::Live(live));
        Ok(())
    }
}

fn controller_with(media: &RecordingMedia, temp_dir: &tempfile::TempDir) -> Controller {
    let (commands, _receiver) = mpsc::unbounded_channel();
    let session = Session::default();
    session.set(Some(Arc::new(crate::demo::FakeServer::new(10))));
    let deps = Deps {
        audio: Box::new(RecordingAudio::default()),
        media: Box::new(media.clone()),
        store: Arc::new(JsonStore::new(temp_dir.path())),
        session,
        ui: Arc::new(RecordingUi::default()),
        commands,
    };
    Controller::new(deps, test_settings(false, false), None, Some(1))
}

#[tokio::test]
async fn media_sessions_see_the_station_live_title_and_the_way_back() {
    let temp_dir = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir: {error}"));
    let media = RecordingMedia::default();
    let mut controller = controller_with(&media, &temp_dir);
    controller.play_now(test_songs(1, 100_000), "s0");
    media.take();

    controller.handle(Command::Radio(RadioCommand::Play(RadioStation {
        id: "7".into(),
        name: "Jazz FM".into(),
        stream_url: "https://jazz.example.com/stream".into(),
        homepage_url: None,
    })));
    controller.handle(Command::Player(PlayerEvent::StreamTitle(Some(
        "Artist - Song".into(),
    ))));
    assert_eq!(
        media.take(),
        [
            MediaCall::Live(true),
            MediaCall::Title("Jazz FM".into()),
            MediaCall::Title("Artist - Song".into()),
        ]
    );

    controller.handle(Command::Radio(RadioCommand::Stop));
    assert_eq!(
        media.take(),
        [MediaCall::Live(false), MediaCall::Title("s0".into())]
    );
    controller.handle(Command::Player(PlayerEvent::Started {
        uid: RADIO_STREAM_UID,
    }));
    assert_eq!(media.take(), []);
}

#[tokio::test]
async fn reconnecting_keeps_the_station_in_the_media_session() {
    let temp_dir = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir: {error}"));
    let media = RecordingMedia::default();
    let mut controller = controller_with(&media, &temp_dir);
    controller.play_now(test_songs(1, 100_000), "s0");
    controller.handle(Command::Radio(RadioCommand::Play(RadioStation {
        id: "7".into(),
        name: "Jazz FM".into(),
        stream_url: "https://jazz.example.com/stream".into(),
        homepage_url: None,
    })));
    media.take();

    controller.handle(Command::Connected {
        config: crate::demo::FakeServer::config_for_demo(),
        credential: None,
        others: Vec::new(),
    });

    assert_eq!(media.take(), []);
}
