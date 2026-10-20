use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use feishin_lyrics::{Lyrics, LyricsQuery, LyricsSource};

const LYRICS_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

#[async_trait]
pub trait LyricsProvider: Send + Sync {
    async fn fetch(
        &self,
        query: &LyricsQuery,
        sources: &[LyricsSource],
    ) -> feishin_lyrics::Result<Option<Lyrics>>;
}

pub struct RemoteLyrics(reqwest::Client);

impl RemoteLyrics {
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(LYRICS_REQUEST_TIMEOUT)
            .build()?;
        Ok(Self(client))
    }
}

#[async_trait]
impl LyricsProvider for RemoteLyrics {
    async fn fetch(
        &self,
        query: &LyricsQuery,
        sources: &[LyricsSource],
    ) -> feishin_lyrics::Result<Option<Lyrics>> {
        feishin_lyrics::fetch(&self.0, query, sources).await
    }
}

pub struct NoRemoteLyrics;

#[async_trait]
impl LyricsProvider for NoRemoteLyrics {
    async fn fetch(
        &self,
        _query: &LyricsQuery,
        _sources: &[LyricsSource],
    ) -> feishin_lyrics::Result<Option<Lyrics>> {
        Ok(None)
    }
}
