//! Desktop integration: MPRIS / media keys and Discord Rich Presence.

mod discord;
mod mpris;

pub use discord::{DEFAULT_DISCORD_CLIENT_ID, DiscordDisplayType, DiscordOptions, DiscordPresence};
pub use mpris::MediaControls;

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
