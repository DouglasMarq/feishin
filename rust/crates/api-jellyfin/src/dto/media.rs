use serde::Deserialize;

use super::library::NamedItem;
use super::null_as_default;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ImageTags {
    pub primary: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct UserData {
    #[serde(deserialize_with = "null_as_default")]
    pub is_favorite: bool,
    #[serde(deserialize_with = "null_as_default")]
    pub play_count: u32,
    pub last_played_date: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct MediaStream {
    #[serde(rename = "Type", deserialize_with = "null_as_default")]
    pub kind: String,
    pub bit_rate: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct MediaSource {
    pub container: Option<String>,
    pub path: Option<String>,
    pub size: Option<u64>,
    #[serde(deserialize_with = "null_as_default")]
    pub media_streams: Vec<MediaStream>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Song {
    #[serde(deserialize_with = "null_as_default")]
    pub id: String,
    #[serde(deserialize_with = "null_as_default")]
    pub name: String,
    pub album: Option<String>,
    pub album_id: Option<String>,
    pub album_primary_image_tag: Option<String>,
    #[serde(deserialize_with = "null_as_default")]
    pub album_artists: Vec<NamedItem>,
    #[serde(deserialize_with = "null_as_default")]
    pub artist_items: Vec<NamedItem>,
    #[serde(deserialize_with = "null_as_default")]
    pub genre_items: Vec<NamedItem>,
    pub image_tags: Option<ImageTags>,
    pub index_number: Option<u32>,
    pub parent_index_number: Option<u32>,
    pub run_time_ticks: Option<u64>,
    pub premiere_date: Option<String>,
    pub production_year: Option<i32>,
    pub date_created: Option<String>,
    #[serde(deserialize_with = "null_as_default")]
    pub media_sources: Vec<MediaSource>,
    pub user_data: Option<UserData>,
    pub normalization_gain: Option<f32>,
    pub album_normalization_gain: Option<f32>,
    #[serde(rename = "LUFS")]
    pub lufs: Option<f32>,
    pub playlist_item_id: Option<String>,
}
