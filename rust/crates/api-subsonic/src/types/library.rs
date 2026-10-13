use serde::Deserialize;

use super::lenient::{lenient_id, lenient_number};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SubsonicGenre {
    pub value: String,
    #[serde(deserialize_with = "lenient_number")]
    pub song_count: Option<f64>,
    #[serde(deserialize_with = "lenient_number")]
    pub album_count: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct SubsonicMusicFolder {
    #[serde(deserialize_with = "lenient_id")]
    pub id: String,
    pub name: String,
}
