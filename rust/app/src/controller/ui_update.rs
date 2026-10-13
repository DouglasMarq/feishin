use feishin_core::model::Song;
use feishin_core::queue::{QueueTrack, RepeatMode, Uid};

use crate::sleep_timer::SleepStatus;

#[derive(Debug, Clone, PartialEq)]
pub struct QueueEntry {
    pub uid: Uid,
    pub song_id: String,
    pub title: String,
    pub artists: String,
    pub duration_ms: u64,
}

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
    Queue {
        entries: Vec<QueueEntry>,
        current: Option<Uid>,
    },
    QueueCurrent(Option<Uid>),
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
