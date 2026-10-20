use std::fmt::{self, Display};

use feishin_api::{ApiError, MusicServer};
use feishin_core::model::Song;
use feishin_lyrics::{LyricLine, LyricsError, LyricsQuery, LyricsSource};
use feishin_storage::settings::LyricsSourceSetting;

use super::lyrics_plan::LyricsStep;
use crate::controller::artist_names;
use crate::lyrics_provider::{LyricsProvider, lyrics_source_label, lyrics_source_setting};

#[derive(Debug)]
pub(super) enum LyricsLookupError {
    Server(ApiError),
    Remote(LyricsError),
}

impl Display for LyricsLookupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Server(error) => Display::fmt(error, formatter),
            Self::Remote(error) => Display::fmt(error, formatter),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SongLyrics {
    pub(super) source: String,
    pub(super) synced: bool,
    pub(super) lines: Vec<LyricLine>,
}

impl SongLyrics {
    pub(super) fn from_text(source: impl Into<String>, text: &str) -> Option<Self> {
        let lines = feishin_lyrics::parse_lrc(text);
        if lines.iter().all(|line| line.text.trim().is_empty()) {
            return None;
        }
        let synced = lines.iter().any(|line| line.time_ms.is_some());
        Some(Self {
            source: source.into(),
            synced,
            lines,
        })
    }

    pub(super) fn line_times_ms(&self) -> Vec<u64> {
        if !self.synced {
            return Vec::new();
        }
        self.lines
            .iter()
            .map(|line| line.time_ms.unwrap_or(0))
            .collect()
    }
}

fn lyrics_query(song: &Song) -> LyricsQuery {
    LyricsQuery {
        title: song.title.clone(),
        artist: artist_names(song),
        album: song.album.clone(),
        duration_ms: Some(song.duration_ms).filter(|duration_ms| *duration_ms > 0),
    }
}

async fn server_lyrics(
    server: &dyn MusicServer,
    song: &Song,
) -> Result<Option<SongLyrics>, LyricsLookupError> {
    let text = server.lyrics(song).await.map_err(|error| {
        tracing::debug!("server lyrics for {}: {error}", song.id);
        LyricsLookupError::Server(error)
    })?;
    Ok(text.and_then(|text| {
        SongLyrics::from_text(lyrics_source_label(LyricsSourceSetting::Server), &text)
    }))
}

pub(super) async fn remote_lyrics_for(
    provider: &dyn LyricsProvider,
    query: &LyricsQuery,
    sources: &[LyricsSource],
) -> Result<Option<SongLyrics>, LyricsLookupError> {
    let found = provider
        .fetch(query, sources)
        .await
        .map_err(LyricsLookupError::Remote)?;
    Ok(found.map(|found| SongLyrics {
        source: lyrics_source_label(lyrics_source_setting(found.source)),
        synced: found.is_synced,
        lines: found.lines,
    }))
}

async fn resolve_step(
    server: &dyn MusicServer,
    provider: &dyn LyricsProvider,
    song: &Song,
    step: &LyricsStep,
) -> Result<Option<SongLyrics>, LyricsLookupError> {
    match step {
        LyricsStep::Server => server_lyrics(server, song).await,
        LyricsStep::Remote(sources) => {
            remote_lyrics_for(provider, &lyrics_query(song), sources).await
        }
    }
}

pub(super) async fn resolve_lyrics(
    server: &dyn MusicServer,
    provider: &dyn LyricsProvider,
    song: &Song,
    plan: &[LyricsStep],
) -> Result<Option<SongLyrics>, LyricsLookupError> {
    let mut first_error = None;
    for step in plan {
        match resolve_step(server, provider, song, step).await {
            Ok(Some(lyrics)) => return Ok(Some(lyrics)),
            Ok(None) => {}
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    first_error.map_or(Ok(None), Err)
}

pub(super) fn active_line(line_times_ms: &[u64], position_ms: u64) -> Option<usize> {
    line_times_ms
        .partition_point(|&time_ms| time_ms <= position_ms)
        .checked_sub(1)
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use feishin_lyrics::Lyrics;

    use super::*;
    use crate::demo::FakeServer;
    use crate::lyrics_provider::NoRemoteLyrics;
    use crate::ui::now_playing::lyrics_plan::lyrics_plan;

    struct GeniusOnly;

    #[async_trait]
    impl LyricsProvider for GeniusOnly {
        async fn fetch(
            &self,
            _query: &LyricsQuery,
            sources: &[LyricsSource],
        ) -> feishin_lyrics::Result<Option<Lyrics>> {
            Ok(sources.contains(&LyricsSource::Genius).then(|| Lyrics {
                source: LyricsSource::Genius,
                is_synced: false,
                lines: feishin_lyrics::parse_lrc("First line\nSecond line"),
            }))
        }
    }

    #[test]
    fn active_line_tracks_the_last_started_line() {
        let line_times_ms = [1_000, 5_000, 5_000, 9_000];
        assert_eq!(active_line(&line_times_ms, 0), None);
        assert_eq!(active_line(&line_times_ms, 1_000), Some(0));
        assert_eq!(active_line(&line_times_ms, 6_000), Some(2));
        assert_eq!(active_line(&line_times_ms, 60_000), Some(3));
        assert_eq!(active_line(&[], 1_000), None);
    }

    #[test]
    fn lyrics_text_maps_to_synced_or_plain() {
        let synced = SongLyrics::from_text("Server", "[00:01.00]One\n[00:02.50]Two").unwrap();
        assert!(synced.synced);
        assert_eq!(synced.line_times_ms(), vec![1_000, 2_500]);
        let plain = SongLyrics::from_text("Server", "One\nTwo").unwrap();
        assert!(!plain.synced);
        assert!(plain.line_times_ms().is_empty());
        assert_eq!(SongLyrics::from_text("Server", "  \n"), None);
    }

    #[tokio::test]
    async fn server_lyrics_come_first() {
        let server = FakeServer::new(100);
        let song = server.song("song-3").await.unwrap();
        let plan = lyrics_plan(&[LyricsSourceSetting::Server]);
        let lyrics = resolve_lyrics(&server, &NoRemoteLyrics, &song, &plan)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(lyrics.source, "Server");
        assert!(lyrics.synced);
        let unplanned = resolve_lyrics(&server, &NoRemoteLyrics, &song, &[]).await;
        assert!(unplanned.unwrap().is_none());
    }

    #[tokio::test]
    async fn given_a_remote_source_the_injected_provider_answers() {
        let server = FakeServer::new(100);
        let song = server.song("song-3").await.unwrap();
        let plan = lyrics_plan(&[LyricsSourceSetting::Genius, LyricsSourceSetting::Server]);
        let lyrics = resolve_lyrics(&server, &GeniusOnly, &song, &plan)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(lyrics.source, "Genius");
        assert_eq!(lyrics.lines.len(), 2);
    }
}
