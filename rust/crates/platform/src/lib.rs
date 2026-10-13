//! Desktop integration: MPRIS / media keys and Discord Rich Presence.

#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("media controls error: {0}")]
    MediaControls(String),
    #[error("discord error: {0}")]
    Discord(String),
}

pub type Result<T> = std::result::Result<T, PlatformError>;

#[derive(Debug, Clone, PartialEq)]
pub enum MediaControlEvent {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    Stop,
    SeekTo {
        position_ms: u64,
    },
    SeekBy {
        offset_ms: i64,
    },
    /// 0.0-1.0.
    SetVolume(f64),
    Raise,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct NowPlaying {
    pub track_id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub cover_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaybackStatus {
    Playing { position_ms: u64 },
    Paused { position_ms: u64 },
    Stopped,
}

/// MPRIS on Linux.
pub struct MediaControls {
    _private: (),
}

impl MediaControls {
    pub fn new(
        _app_name: &str,
        _on_event: Box<dyn Fn(MediaControlEvent) + Send + Sync>,
    ) -> Result<Self> {
        todo!()
    }

    pub fn set_metadata(&mut self, _now_playing: &NowPlaying) -> Result<()> {
        todo!()
    }

    pub fn set_status(&mut self, _status: PlaybackStatus) -> Result<()> {
        todo!()
    }

    /// 0.0-1.0.
    pub fn set_volume(&mut self, _volume: f64) -> Result<()> {
        todo!()
    }
}

pub struct DiscordPresence {
    _private: (),
}

impl DiscordPresence {
    pub fn connect(_client_id: &str) -> Result<Self> {
        todo!()
    }

    pub fn set_activity(
        &mut self,
        _now_playing: &NowPlaying,
        _position_ms: Option<u64>,
        _paused: bool,
    ) -> Result<()> {
        todo!()
    }

    pub fn clear(&mut self) -> Result<()> {
        todo!()
    }
}
