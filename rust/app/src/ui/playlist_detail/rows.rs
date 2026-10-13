use feishin_core::model::Song;

use crate::ui::playlists::song_count_label;
use crate::ui::tracks::track_row;
use crate::ui::{PlaylistSongRow, format_duration};

pub fn header_info(songs: &[Song], owner: Option<&str>) -> String {
    let duration: u64 = songs.iter().map(|song| song.duration_ms).sum();
    let mut parts = vec![song_count_label(songs.len())];
    parts.extend((!songs.is_empty()).then(|| format_duration(duration)));
    parts.extend(
        owner
            .filter(|owner| !owner.is_empty())
            .map(|owner| format!("by {owner}")),
    );
    parts.join(" • ")
}

pub fn song_row(song: &Song, index: usize) -> PlaylistSongRow {
    PlaylistSongRow {
        track: track_row(song, index),
        album_id: song.album_id.as_deref().unwrap_or_default().into(),
        artist_id: song
            .artists
            .first()
            .map(|artist| artist.id.as_str())
            .unwrap_or_default()
            .into(),
        selected: false,
    }
}

pub fn song_rows(songs: &[Song]) -> Vec<PlaylistSongRow> {
    songs
        .iter()
        .enumerate()
        .map(|(index, song)| song_row(song, index))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::{FakeServer, test_song};

    #[test]
    fn header_info_sums_songs() {
        let songs = [test_song("a", 60_000), test_song("b", 90_000)];
        assert_eq!(
            header_info(&songs, Some("demo")),
            "2 songs • 2:30 • by demo"
        );
        assert_eq!(header_info(&[], None), "0 songs");
    }

    #[test]
    fn song_rows_link_album_and_artist() {
        let server = FakeServer::new(50);
        let row = song_row(&server.song_at(13), 0);
        assert_eq!(row.album_id, "album-1");
        assert!(row.artist_id.starts_with("artist-"));
        assert_eq!(row.track.index, 1);
        assert!(!row.selected);
    }
}
