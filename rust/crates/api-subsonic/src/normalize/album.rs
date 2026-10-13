use feishin_core::model::{Album, Song};

use super::artists::{artist_refs, genre_names};
use super::numbers::{
    capped_rating, non_empty_text, non_negative_u32, nonzero_year, seconds_to_milliseconds,
};
use super::song::song;
use crate::types::{ReleaseDate, SubsonicAlbum};

fn is_complete_release_date(release_date: &ReleaseDate) -> bool {
    release_date.year.is_some() && release_date.month.is_some() && release_date.day.is_some()
}

fn release_year(item: &SubsonicAlbum) -> Option<i32> {
    match &item.release_date {
        Some(release_date) if is_complete_release_date(release_date) => {
            nonzero_year(release_date.year).or_else(|| nonzero_year(item.year))
        }
        _ => nonzero_year(item.year),
    }
}

pub fn album(item: SubsonicAlbum, server_id: &str) -> (Album, Vec<Song>) {
    let year = release_year(&item);
    let album_artists = artist_refs(
        item.artists,
        item.artist_id.as_deref(),
        item.artist.as_deref(),
        None,
    );
    let songs = item
        .song
        .unwrap_or_default()
        .into_iter()
        .map(|entry| song(entry, server_id))
        .collect();
    let album = Album {
        id: item.id,
        server_id: server_id.to_owned(),
        name: item.name.or(item.title).or(item.album).unwrap_or_default(),
        album_artists,
        artists: Vec::new(),
        year,
        genres: genre_names(item.genres, item.genre),
        song_count: non_negative_u32(item.song_count).unwrap_or(0),
        duration_ms: seconds_to_milliseconds(item.duration),
        cover_art_id: non_empty_text(item.cover_art),
        play_count: non_negative_u32(item.play_count).unwrap_or(0),
        user_rating: capped_rating(item.user_rating),
        favorite: item.starred.is_starred(),
        created_at: item.created,
        last_played_at: None,
    };
    (album, songs)
}

pub fn album_without_songs(item: SubsonicAlbum, server_id: &str) -> Album {
    album(item, server_id).0
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn album_prefers_full_release_date_year() {
        let item: SubsonicAlbum = serde_json::from_value(json!({
            "id": "a1", "name": "N", "artist": "X", "artistId": "x", "year": 1999,
            "releaseDate": {"year": 2005, "month": 3, "day": 1}, "songCount": 3, "duration": 100,
            "song": [{"id": "s1", "title": "one"}]
        }))
        .unwrap_or_default();
        let (album, songs) = album(item, "srv");
        assert_eq!(album.year, Some(2005));
        assert_eq!(album.song_count, 3);
        assert_eq!(album.album_artists[0].id, "x");
        assert_eq!(songs.len(), 1);
    }
}
