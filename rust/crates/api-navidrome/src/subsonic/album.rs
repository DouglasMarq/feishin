use feishin_core::model::Album;
use serde::Deserialize;

use crate::conversions::{non_empty, rating, seconds_to_ms};
use crate::lenient::{optional_f64, optional_i32, optional_string, optional_u32, string, truthy};
use crate::subsonic::references::{
    SubsonicGenreName, SubsonicReference, artist_refs_or_single, genre_names,
};
use crate::subsonic::song::SubsonicSong;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct SubsonicReleaseDate {
    #[serde(deserialize_with = "optional_i32")]
    pub year: Option<i32>,
    #[serde(deserialize_with = "optional_u32")]
    pub month: Option<u32>,
    #[serde(deserialize_with = "optional_u32")]
    pub day: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct SubsonicAlbum {
    #[serde(deserialize_with = "string")]
    pub id: String,
    #[serde(deserialize_with = "string")]
    pub name: String,
    #[serde(deserialize_with = "optional_string")]
    pub artist: Option<String>,
    #[serde(deserialize_with = "optional_string")]
    pub artist_id: Option<String>,
    pub artists: Option<Vec<SubsonicReference>>,
    #[serde(deserialize_with = "optional_string")]
    pub cover_art: Option<String>,
    #[serde(deserialize_with = "optional_u32")]
    pub song_count: Option<u32>,
    #[serde(deserialize_with = "optional_f64")]
    pub duration: Option<f64>,
    #[serde(deserialize_with = "optional_i32")]
    pub year: Option<i32>,
    pub release_date: Option<SubsonicReleaseDate>,
    #[serde(deserialize_with = "optional_string")]
    pub genre: Option<String>,
    pub genres: Option<Vec<SubsonicGenreName>>,
    #[serde(deserialize_with = "optional_u32")]
    pub play_count: Option<u32>,
    #[serde(deserialize_with = "optional_u32")]
    pub user_rating: Option<u32>,
    #[serde(deserialize_with = "truthy")]
    pub starred: bool,
    #[serde(deserialize_with = "optional_string")]
    pub created: Option<String>,
    #[serde(deserialize_with = "optional_string")]
    pub played: Option<String>,
    pub song: Option<Vec<SubsonicSong>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct AlbumResponse {
    pub album: Option<SubsonicAlbum>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct AlbumList {
    pub album: Vec<SubsonicAlbum>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct AlbumList2 {
    pub album_list2: Option<AlbumList>,
}

pub(crate) fn normalize_album(album: SubsonicAlbum, server_id: &str) -> Album {
    let year = album_year(&album);

    Album {
        album_artists: artist_refs_or_single(
            album.artists.as_ref(),
            album.artist_id.as_deref(),
            album.artist.as_deref(),
        ),
        artists: Vec::new(),
        id: album.id,
        server_id: server_id.to_owned(),
        name: album.name,
        year,
        genres: genre_names(album.genres, album.genre),
        song_count: album.song_count.unwrap_or(0),
        duration_ms: seconds_to_ms(album.duration),
        cover_art_id: non_empty(album.cover_art),
        play_count: album.play_count.unwrap_or(0),
        user_rating: rating(album.user_rating),
        favorite: album.starred,
        created_at: album.created,
        last_played_at: non_empty(album.played),
    }
}

fn album_year(album: &SubsonicAlbum) -> Option<i32> {
    album
        .release_date
        .as_ref()
        .and_then(complete_release_year)
        .or(album.year.filter(|year| *year != 0))
}

fn complete_release_year(release_date: &SubsonicReleaseDate) -> Option<i32> {
    match (release_date.year, release_date.month, release_date.day) {
        (Some(year), Some(_), Some(_)) if year > 0 => Some(year),
        _ => None,
    }
}
