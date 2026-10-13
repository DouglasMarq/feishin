//! External lyrics providers (port of `src/main/features/core/lyrics`) and LRC parsing.

#[derive(Debug, thiserror::Error)]
pub enum LyricsError {
    #[error("network error: {0}")]
    Network(String),
    #[error("unexpected response: {0}")]
    Decode(String),
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
    /// `None` for unsynced lyrics.
    pub time_ms: Option<u64>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lyrics {
    pub source: LyricsSource,
    pub synced: bool,
    pub lines: Vec<LyricLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsQuery {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
}

/// Tries `sources` in order and returns the first match, preferring synced lyrics within a source.
pub async fn fetch(
    _client: &reqwest::Client,
    _query: &LyricsQuery,
    _sources: &[LyricsSource],
) -> Result<Option<Lyrics>> {
    todo!()
}

/// Parses LRC (`[mm:ss.xx] text`, multiple timestamps per line, offset tag). Plain text becomes unsynced lines.
pub fn parse_lrc(_text: &str) -> Vec<LyricLine> {
    todo!()
}
