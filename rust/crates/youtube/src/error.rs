use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, thiserror::Error)]
pub enum YouTubeError {
    #[error("yt-dlp is not installed")]
    NotInstalled,
    #[error("yt-dlp could not be started: {0}")]
    Launch(#[source] Arc<std::io::Error>),
    #[error("yt-dlp did not answer within {} seconds", .0.as_secs())]
    TimedOut(Duration),
    #[error("yt-dlp failed: {0}")]
    Failed(String),
    #[error("yt-dlp printed no audio address")]
    NoAudioUrl,
    #[error("yt-dlp printed no video details")]
    NoVideoDetails,
    #[error("\"{0}\" is not a YouTube video id")]
    InvalidVideoId(String),
}

impl YouTubeError {
    pub fn user_message(&self) -> String {
        match self {
            Self::NotInstalled => "Install yt-dlp to play YouTube tracks".to_owned(),
            other => format!("Could not play from YouTube: {other}"),
        }
    }
}
