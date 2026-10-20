mod discord;
#[cfg(all(unix, not(target_os = "macos")))]
mod freedesktop;
#[cfg(target_os = "macos")]
mod macos;

pub use discord::{DEFAULT_DISCORD_CLIENT_ID, DiscordDisplayType, DiscordOptions, DiscordPresence};
#[cfg(all(unix, not(target_os = "macos")))]
pub use freedesktop::{MediaControls, SleepInhibitor, current_color_scheme, watch_color_scheme};
#[cfg(target_os = "macos")]
pub use macos::{MediaControls, SleepInhibitor, current_color_scheme, watch_color_scheme};

#[derive(Debug, Clone, Copy)]
pub struct MediaIdentity<'text> {
    pub name: &'text str,
    pub desktop_entry: &'text str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SystemColorScheme {
    Dark,
    Light,
    #[default]
    NoPreference,
}

#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("media controls error: {0}")]
    MediaControls(String),
    #[error("discord error: {0}")]
    Discord(String),
    #[error("desktop settings error: {0}")]
    DesktopSettings(String),
    #[error("sleep inhibitor error: {0}")]
    SleepInhibitor(String),
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
    SeekTo { position_ms: u64 },
    SeekBy { offset_ms: i64 },
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
