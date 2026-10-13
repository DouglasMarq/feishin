use serde::Deserialize;

use super::lenient::{lenient_id, lenient_number, lenient_optional_id};
use super::starred::Starred;

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SimpleArtist {
    #[serde(deserialize_with = "lenient_id")]
    pub id: String,
    pub name: String,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Contributor {
    pub artist: SimpleArtist,
    pub role: String,
    pub sub_role: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct GenreItem {
    pub name: String,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ReplayGain {
    #[serde(deserialize_with = "lenient_number")]
    pub album_gain: Option<f64>,
    #[serde(deserialize_with = "lenient_number")]
    pub album_peak: Option<f64>,
    #[serde(deserialize_with = "lenient_number")]
    pub track_gain: Option<f64>,
    #[serde(deserialize_with = "lenient_number")]
    pub track_peak: Option<f64>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SubsonicSong {
    #[serde(deserialize_with = "lenient_id")]
    pub id: String,
    pub title: String,
    pub album: Option<String>,
    #[serde(deserialize_with = "lenient_optional_id")]
    pub album_id: Option<String>,
    pub artist: Option<String>,
    #[serde(deserialize_with = "lenient_optional_id")]
    pub artist_id: Option<String>,
    pub artists: Option<Vec<SimpleArtist>>,
    pub album_artists: Option<Vec<SimpleArtist>>,
    pub contributors: Option<Vec<Contributor>>,
    #[serde(deserialize_with = "lenient_number")]
    pub duration: Option<f64>,
    #[serde(deserialize_with = "lenient_number")]
    pub track: Option<f64>,
    #[serde(deserialize_with = "lenient_number")]
    pub disc_number: Option<f64>,
    #[serde(deserialize_with = "lenient_number")]
    pub year: Option<f64>,
    #[serde(deserialize_with = "lenient_number")]
    pub bit_rate: Option<f64>,
    #[serde(deserialize_with = "lenient_number")]
    pub play_count: Option<f64>,
    #[serde(deserialize_with = "lenient_number")]
    pub user_rating: Option<f64>,
    #[serde(deserialize_with = "lenient_number")]
    pub size: Option<f64>,
    pub content_type: Option<String>,
    pub suffix: Option<String>,
    #[serde(deserialize_with = "lenient_optional_id")]
    pub cover_art: Option<String>,
    pub created: Option<String>,
    pub genre: Option<String>,
    pub genres: Option<Vec<GenreItem>>,
    pub path: Option<String>,
    pub replay_gain: Option<ReplayGain>,
    pub starred: Starred,
}
