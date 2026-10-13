mod fuzzy;
mod genius;
mod http;
mod lrc;
mod lrclib;
mod model;
mod netease;
mod source_chain;

pub use model::{LyricLine, Lyrics, LyricsError, LyricsQuery, LyricsSource, Result};

pub async fn fetch(
    client: &reqwest::Client,
    query: &LyricsQuery,
    sources: &[LyricsSource],
) -> Result<Option<Lyrics>> {
    source_chain::fetch_from(client, query, sources, &source_chain::Endpoints::default()).await
}

pub fn parse_lrc(text: &str) -> Vec<LyricLine> {
    lrc::parse_lrc(text)
}
