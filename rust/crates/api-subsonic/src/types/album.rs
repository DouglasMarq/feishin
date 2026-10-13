use serde::Deserialize;

use super::lenient::{lenient_id, lenient_number, lenient_optional_id};
use super::song::{GenreItem, SimpleArtist, SubsonicSong};
use super::starred::Starred;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ReleaseDate {
    #[serde(deserialize_with = "lenient_number")]
    pub year: Option<f64>,
    #[serde(deserialize_with = "lenient_number")]
    pub month: Option<f64>,
    #[serde(deserialize_with = "lenient_number")]
    pub day: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SubsonicAlbum {
    #[serde(deserialize_with = "lenient_id")]
    pub id: String,
    pub name: Option<String>,
    pub title: Option<String>,
    pub album: Option<String>,
    pub artist: Option<String>,
    #[serde(deserialize_with = "lenient_optional_id")]
    pub artist_id: Option<String>,
    pub artists: Option<Vec<SimpleArtist>>,
    #[serde(deserialize_with = "lenient_optional_id")]
    pub cover_art: Option<String>,
    pub created: Option<String>,
    #[serde(deserialize_with = "lenient_number")]
    pub duration: Option<f64>,
    pub genre: Option<String>,
    pub genres: Option<Vec<GenreItem>>,
    #[serde(deserialize_with = "lenient_number")]
    pub song_count: Option<f64>,
    #[serde(deserialize_with = "lenient_number")]
    pub play_count: Option<f64>,
    #[serde(deserialize_with = "lenient_number")]
    pub user_rating: Option<f64>,
    #[serde(deserialize_with = "lenient_number")]
    pub year: Option<f64>,
    pub release_date: Option<ReleaseDate>,
    pub starred: Starred,
    pub song: Option<Vec<SubsonicSong>>,
}
