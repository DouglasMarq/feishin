use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use feishin_lyrics::{Lyrics, LyricsQuery, LyricsSource};
use feishin_storage::settings::LyricsSourceSetting;

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

pub fn remote_lyrics_source(source: LyricsSourceSetting) -> Option<LyricsSource> {
    match source {
        LyricsSourceSetting::Server => None,
        LyricsSourceSetting::Lrclib => Some(LyricsSource::Lrclib),
        LyricsSourceSetting::NetEase => Some(LyricsSource::NetEase),
        LyricsSourceSetting::Genius => Some(LyricsSource::Genius),
    }
}

pub fn lyrics_source_setting(source: LyricsSource) -> LyricsSourceSetting {
    match source {
        LyricsSource::Lrclib => LyricsSourceSetting::Lrclib,
        LyricsSource::NetEase => LyricsSourceSetting::NetEase,
        LyricsSource::Genius => LyricsSourceSetting::Genius,
    }
}

pub fn lyrics_source_label(source: LyricsSourceSetting) -> String {
    match source {
        LyricsSourceSetting::Server => tr!("Server"),
        LyricsSourceSetting::Lrclib => "LRCLIB".to_owned(),
        LyricsSourceSetting::NetEase => "NetEase".to_owned(),
        LyricsSourceSetting::Genius => "Genius".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_each_remote_source_its_setting_maps_back_to_it() {
        for source in [
            LyricsSource::Lrclib,
            LyricsSource::NetEase,
            LyricsSource::Genius,
        ] {
            assert_eq!(
                remote_lyrics_source(lyrics_source_setting(source)),
                Some(source)
            );
        }
    }

    #[test]
    fn given_the_server_source_it_has_no_remote_source() {
        assert_eq!(remote_lyrics_source(LyricsSourceSetting::Server), None);
    }

    #[test]
    fn given_each_lyrics_source_its_label_is_the_product_name() {
        let labels: Vec<_> = LyricsSourceSetting::ALL
            .into_iter()
            .map(lyrics_source_label)
            .collect();
        assert_eq!(labels, ["Server", "LRCLIB", "NetEase", "Genius"]);
    }
}
