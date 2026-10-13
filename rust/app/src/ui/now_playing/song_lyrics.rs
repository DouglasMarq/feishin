use std::sync::LazyLock;

use feishin_api::MusicServer;
use feishin_core::model::Song;
use feishin_lyrics::{LyricLine, LyricsQuery, LyricsSource};

use super::lyrics_plan::LyricsStep;
use crate::controller::artist_names;

static HTTP: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

const SERVER_SOURCE_LABEL: &str = "Server";

fn source_label(source: LyricsSource) -> &'static str {
    match source {
        LyricsSource::Lrclib => "LRCLIB",
        LyricsSource::NetEase => "NetEase",
        LyricsSource::Genius => "Genius",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SongLyrics {
    pub(super) source: &'static str,
    pub(super) synced: bool,
    pub(super) lines: Vec<LyricLine>,
}

impl SongLyrics {
    pub(super) fn from_text(source: &'static str, text: &str) -> Option<Self> {
        let lines = feishin_lyrics::parse_lrc(text);
        if lines.iter().all(|line| line.text.trim().is_empty()) {
            return None;
        }
        let synced = lines.iter().any(|line| line.time_ms.is_some());
        Some(Self {
            source,
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
) -> Result<Option<SongLyrics>, String> {
    let text = server.lyrics(song).await.map_err(|error| {
        tracing::debug!("server lyrics for {}: {error}", song.id);
        error.to_string()
    })?;
    Ok(text.and_then(|text| SongLyrics::from_text(SERVER_SOURCE_LABEL, &text)))
}

async fn remote_lyrics(
    song: &Song,
    sources: &[LyricsSource],
) -> Result<Option<SongLyrics>, String> {
    let found = feishin_lyrics::fetch(&HTTP, &lyrics_query(song), sources)
        .await
        .map_err(|error| error.to_string())?;
    Ok(found.map(|found| SongLyrics {
        source: source_label(found.source),
        synced: found.synced,
        lines: found.lines,
    }))
}

async fn resolve_step(
    server: &dyn MusicServer,
    song: &Song,
    step: &LyricsStep,
) -> Result<Option<SongLyrics>, String> {
    match step {
        LyricsStep::Server => server_lyrics(server, song).await,
        LyricsStep::Remote(sources) => remote_lyrics(song, sources).await,
    }
}

pub(super) async fn resolve_lyrics(
    server: &dyn MusicServer,
    song: &Song,
    plan: &[LyricsStep],
) -> Result<Option<SongLyrics>, String> {
    let mut first_error = None;
    for step in plan {
        match resolve_step(server, song, step).await {
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
    use super::*;
    use crate::demo::FakeServer;
    use crate::ui::now_playing::lyrics_plan::{lyrics_plan, source_names};

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
        let lyrics = resolve_lyrics(&server, &song, &lyrics_plan(&source_names(&["server"])))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(lyrics.source, "Server");
        assert!(lyrics.synced);
        assert!(resolve_lyrics(&server, &song, &[]).await.unwrap().is_none());
    }
}
