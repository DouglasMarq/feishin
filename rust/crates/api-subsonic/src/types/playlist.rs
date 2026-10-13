use serde::Deserialize;

use super::lenient::{lenient_id, lenient_number, lenient_optional_id};
use super::song::SubsonicSong;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SubsonicPlaylist {
    #[serde(deserialize_with = "lenient_id")]
    pub id: String,
    pub name: String,
    pub comment: Option<String>,
    pub owner: Option<String>,
    pub public: Option<bool>,
    #[serde(deserialize_with = "lenient_number")]
    pub song_count: Option<f64>,
    #[serde(deserialize_with = "lenient_number")]
    pub duration: Option<f64>,
    #[serde(deserialize_with = "lenient_optional_id")]
    pub cover_art: Option<String>,
    pub created: Option<String>,
    pub changed: Option<String>,
    pub entry: Option<Vec<SubsonicSong>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct PlaylistContainer {
    pub playlist: Option<Vec<SubsonicPlaylist>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Playlists {
    pub playlists: Option<PlaylistContainer>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct PlaylistResponse {
    pub playlist: Option<SubsonicPlaylist>,
}
