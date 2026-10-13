use feishin_core::model::Song;
use feishin_core::queue::{QueueTrack, QueueView, RepeatMode};

use crate::sleep_timer::SleepStatus;

#[derive(Debug, Clone, PartialEq)]
pub enum UiUpdate {
    NowPlaying(Option<Box<QueueTrack>>),
    Playback {
        playing: bool,
        position_ms: u64,
        duration_ms: u64,
    },
    Volume {
        volume: u8,
        muted: bool,
    },
    Modes {
        shuffle: bool,
        repeat: RepeatMode,
    },
    Queue(QueueView),
    Error(String),
    Raise,
    Quit,
}

pub trait UiSink: Send + Sync {
    fn send(&self, update: UiUpdate);
    fn sleep_timer(&self, _status: Option<SleepStatus>) {}
}

pub fn artist_names(song: &Song) -> String {
    song.artists
        .iter()
        .map(|artist| artist.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}
