mod discord;
#[cfg(all(unix, not(target_os = "macos")))]
mod freedesktop;
mod image_url;
#[cfg(target_os = "macos")]
mod macos;
mod tray;
mod url_opener;

use std::path::PathBuf;

pub use discord::{DEFAULT_DISCORD_CLIENT_ID, DiscordDisplayType, DiscordOptions, DiscordPresence};
#[cfg(all(unix, not(target_os = "macos")))]
pub use freedesktop::{MediaControls, SleepInhibitor, current_color_scheme, watch_color_scheme};
#[cfg(target_os = "macos")]
pub use macos::{MediaControls, SleepInhibitor, current_color_scheme, watch_color_scheme};
pub use tray::{Tray, TrayLabels};
pub use url_opener::open_url;

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

pub type ErrorCause = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("media controls error: {0}")]
    MediaControls(#[source] ErrorCause),
    #[error("discord error: {0}")]
    Discord(#[source] ErrorCause),
    #[error("desktop settings error: {0}")]
    DesktopSettings(#[source] ErrorCause),
    #[error("sleep inhibitor error: {0}")]
    SleepInhibitor(#[source] ErrorCause),
    #[error("could not open the link: {0}")]
    OpenUrl(#[source] ErrorCause),
    #[error("tray icon error: {0}")]
    Tray(#[source] ErrorCause),
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
    pub cover_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaybackStatus {
    Playing { position_ms: u64 },
    Paused { position_ms: u64 },
    Stopped,
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;

    #[test]
    fn given_a_wrapped_failure_the_source_is_the_original_error() {
        let refused = std::io::Error::from(std::io::ErrorKind::ConnectionRefused);
        let error = PlatformError::Discord(Box::new(refused));
        let source = error.source().unwrap();
        assert!(source.downcast_ref::<std::io::Error>().is_some());
        assert_eq!(error.to_string(), format!("discord error: {source}"));
    }
}
