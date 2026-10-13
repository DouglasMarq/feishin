use feishin_core::model::{Playlist, Song};

use crate::ui::playlists::song_count_label;
use crate::ui::tracks::track_row;
use crate::ui::{PlaylistSongRow, TrackRow, clamp_i32, format_duration};

pub struct HeaderSummary<'playlist> {
    pub song_count: usize,
    pub duration_ms: Option<u64>,
    pub owner: Option<&'playlist str>,
}

impl<'playlist> HeaderSummary<'playlist> {
    pub fn new(metadata: Option<&'playlist Playlist>, loaded_total: usize) -> Self {
        Self {
            song_count: metadata
                .map(|playlist| playlist.song_count as usize)
                .filter(|count| *count > 0)
                .unwrap_or(loaded_total),
            duration_ms: metadata.map(|playlist| playlist.duration_ms),
            owner: metadata.and_then(|playlist| playlist.owner.as_deref()),
        }
    }
}

pub fn header_info(summary: &HeaderSummary) -> String {
    let mut parts = vec![song_count_label(summary.song_count)];
    parts.extend(
        summary
            .duration_ms
            .filter(|duration| *duration > 0 && summary.song_count > 0)
            .map(format_duration),
    );
    parts.extend(
        summary
            .owner
            .filter(|owner| !owner.is_empty())
            .map(|owner| format!("by {owner}")),
    );
    parts.join(" • ")
}

pub fn song_row(song: &Song, index: usize, selected: bool) -> PlaylistSongRow {
    PlaylistSongRow {
        track: track_row(song, index),
        album_id: song.album_id.as_deref().unwrap_or_default().into(),
        artist_id: song
            .artists
            .first()
            .map(|artist| artist.id.as_str())
            .unwrap_or_default()
            .into(),
        selected,
    }
}

pub fn placeholder_row(index: usize, selected: bool) -> PlaylistSongRow {
    PlaylistSongRow {
        track: TrackRow {
            index: clamp_i32(index as u64 + 1),
            ..TrackRow::default()
        },
        selected,
        ..PlaylistSongRow::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::FakeServer;

    fn summary(song_count: usize, duration_ms: Option<u64>) -> HeaderSummary<'static> {
        HeaderSummary {
            song_count,
            duration_ms,
            owner: Some("demo"),
        }
    }

    #[test]
    fn header_info_reads_the_playlist_totals() {
        assert_eq!(
            header_info(&summary(2, Some(150_000))),
            "2 songs • 2:30 • by demo"
        );
        assert_eq!(header_info(&summary(3, None)), "3 songs • by demo");
        let empty = HeaderSummary {
            song_count: 0,
            duration_ms: Some(0),
            owner: None,
        };
        assert_eq!(header_info(&empty), "0 songs");
    }

    #[test]
    fn header_summary_prefers_the_playlist_metadata() {
        let playlist = Playlist {
            id: "playlist-1".into(),
            server_id: "demo".into(),
            name: "Mix".into(),
            comment: None,
            owner: Some("demo".into()),
            public: false,
            song_count: 12,
            duration_ms: 600_000,
            cover_art_id: None,
            created_at: None,
            updated_at: None,
        };
        let summary = HeaderSummary::new(Some(&playlist), 999);
        assert_eq!(summary.song_count, 12);
        assert_eq!(summary.duration_ms, Some(600_000));
        assert_eq!(summary.owner, Some("demo"));
        assert_eq!(HeaderSummary::new(None, 7).song_count, 7);
    }

    #[test]
    fn song_rows_link_album_and_artist() {
        let server = FakeServer::new(50);
        let row = song_row(&server.song_at(13), 0, true);
        assert_eq!(row.album_id, "album-1");
        assert!(row.artist_id.starts_with("artist-"));
        assert_eq!(row.track.index, 1);
        assert!(row.selected);
        let placeholder = placeholder_row(4, false);
        assert_eq!(placeholder.track.index, 5);
        assert!(!placeholder.track.loaded);
    }
}
