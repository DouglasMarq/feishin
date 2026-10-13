use feishin_core::model::Song;

use crate::controller::artist_names;
use crate::ui::{TrackRow, clamp_i32, format_duration};

pub(super) fn primary_artist_id(song: &Song) -> Option<&str> {
    song.artists
        .first()
        .or_else(|| song.album_artists.first())
        .map(|artist| artist.id.as_str())
}

pub fn track_row(song: &Song, index: usize) -> TrackRow {
    TrackRow {
        id: song.id.as_str().into(),
        index: clamp_i32(index as u64 + 1),
        title: song.title.as_str().into(),
        artists: artist_names(song).into(),
        album: song.album.as_deref().unwrap_or_default().into(),
        duration: format_duration(song.duration_ms).into(),
        year: song
            .year
            .map(|year| year.to_string())
            .unwrap_or_default()
            .into(),
        plays: song.play_count.to_string().into(),
        loaded: true,
        playing: false,
        album_id: song.album_id.as_deref().unwrap_or_default().into(),
        artist_id: primary_artist_id(song).unwrap_or_default().into(),
        favorite: song.favorite,
        rating: i32::from(song.user_rating.unwrap_or_default()),
        selected: false,
    }
}
