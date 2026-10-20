pub type ErrorCause = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, thiserror::Error)]
pub enum LyricsError {
    #[error("network error: {0}")]
    Network(#[source] ErrorCause),
    #[error("unexpected response: {0}")]
    Decode(#[source] ErrorCause),
}

pub type Result<T> = std::result::Result<T, LyricsError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LyricsSource {
    Lrclib,
    NetEase,
    Genius,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricLine {
    pub time_ms: Option<u64>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lyrics {
    pub source: LyricsSource,
    pub is_synced: bool,
    pub lines: Vec<LyricLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsQuery {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
}

impl LyricsQuery {
    pub(crate) fn has_no_search_terms(&self) -> bool {
        self.title.is_empty() && self.artist.is_empty()
    }
}
