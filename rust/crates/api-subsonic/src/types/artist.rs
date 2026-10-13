use serde::Deserialize;

use super::album::SubsonicAlbum;
use super::lenient::{lenient_id, lenient_number, lenient_optional_id};
use super::starred::Starred;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SubsonicArtist {
    #[serde(deserialize_with = "lenient_id")]
    pub id: String,
    pub name: String,
    #[serde(deserialize_with = "lenient_number")]
    pub album_count: Option<f64>,
    #[serde(deserialize_with = "lenient_optional_id")]
    pub cover_art: Option<String>,
    #[serde(deserialize_with = "lenient_number")]
    pub user_rating: Option<f64>,
    pub starred: Starred,
    pub album: Option<Vec<SubsonicAlbum>>,
}
