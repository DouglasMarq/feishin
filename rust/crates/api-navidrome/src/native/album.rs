use feishin_core::model::Album;
use serde::Deserialize;

use crate::conversions::{non_empty, partial_iso_year, play_date, rating, seconds_to_ms};
use crate::lenient::{optional_f64, optional_i32, optional_string, optional_u32, string, truthy};
use crate::native::related_artists::{
    ArtistFields, NativeReference, Participants, Remixers, genre_names, related_artists,
};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct NativeAlbum {
    #[serde(deserialize_with = "string")]
    pub id: String,
    #[serde(deserialize_with = "string")]
    pub name: String,
    #[serde(deserialize_with = "optional_string")]
    pub album_artist: Option<String>,
    #[serde(deserialize_with = "optional_string")]
    pub album_artist_id: Option<String>,
    #[serde(deserialize_with = "optional_string")]
    pub artist: Option<String>,
    #[serde(deserialize_with = "optional_string")]
    pub artist_id: Option<String>,
    pub participants: Option<Participants>,
    #[serde(deserialize_with = "optional_string")]
    pub cover_art_id: Option<String>,
    #[serde(deserialize_with = "optional_u32")]
    pub song_count: Option<u32>,
    #[serde(deserialize_with = "optional_f64")]
    pub duration: Option<f64>,
    pub genres: Option<Vec<NativeReference>>,
    #[serde(deserialize_with = "optional_i32")]
    pub min_year: Option<i32>,
    #[serde(deserialize_with = "optional_string")]
    pub date: Option<String>,
    #[serde(deserialize_with = "optional_string")]
    pub release_date: Option<String>,
    #[serde(deserialize_with = "optional_u32")]
    pub play_count: Option<u32>,
    #[serde(deserialize_with = "optional_u32")]
    pub rating: Option<u32>,
    #[serde(deserialize_with = "truthy")]
    pub starred: bool,
    #[serde(deserialize_with = "optional_string")]
    pub created_at: Option<String>,
    #[serde(deserialize_with = "optional_string")]
    pub play_date: Option<String>,
}

pub(crate) fn normalize_album(album: NativeAlbum, server_id: &str) -> Album {
    let related = related_artists(&artist_fields(&album), Remixers::Exclude);
    let year = album_year(&album);

    Album {
        cover_art_id: Some(non_empty(album.cover_art_id).unwrap_or_else(|| album.id.clone())),
        id: album.id,
        server_id: server_id.to_owned(),
        name: album.name,
        album_artists: related.album_artists,
        artists: related.artists,
        year,
        genres: genre_names(album.genres.as_ref()),
        song_count: album.song_count.unwrap_or(0),
        duration_ms: seconds_to_ms(album.duration),
        play_count: album.play_count.unwrap_or(0),
        user_rating: rating(album.rating),
        favorite: album.starred,
        created_at: album.created_at,
        last_played_at: play_date(album.play_date),
    }
}

fn artist_fields(album: &NativeAlbum) -> ArtistFields<'_> {
    ArtistFields {
        participants: album.participants.as_ref(),
        album_artist_id: album.album_artist_id.as_deref(),
        album_artist: album.album_artist.as_deref(),
        artist_id: album.artist_id.as_deref(),
        artist: album.artist.as_deref(),
    }
}

fn album_year(album: &NativeAlbum) -> Option<i32> {
    partial_iso_year(album.release_date.as_deref())
        .or_else(|| partial_iso_year(album.date.as_deref()))
        .or(album.min_year.filter(|year| *year != 0))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn album_without_participants_falls_back_to_fields() {
        let raw: NativeAlbum = serde_json::from_value(json!({
            "id": "al1",
            "name": "Album",
            "albumArtist": "AA",
            "albumArtistId": "aa1",
            "minYear": 2004,
            "releaseDate": "",
            "songCount": 3
        }))
        .expect("fixture");
        let album = normalize_album(raw, "srv");
        assert_eq!(album.album_artists[0].name, "AA");
        assert!(album.artists.is_empty());
        assert_eq!(album.year, Some(2004));
        assert_eq!(album.cover_art_id.as_deref(), Some("al1"));
    }
}
