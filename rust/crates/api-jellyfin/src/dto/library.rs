use serde::Deserialize;

use super::media::{ImageTags, UserData};
use super::null_as_default;

#[derive(Debug, Deserialize)]
#[serde(
    rename_all = "PascalCase",
    bound(deserialize = "Item: Deserialize<'de>")
)]
pub struct ItemList<Item> {
    #[serde(default = "Vec::new", deserialize_with = "null_as_default")]
    pub items: Vec<Item>,
    #[serde(default)]
    pub total_record_count: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct NamedItem {
    #[serde(deserialize_with = "null_as_default")]
    pub id: String,
    #[serde(deserialize_with = "null_as_default")]
    pub name: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Album {
    #[serde(deserialize_with = "null_as_default")]
    pub id: String,
    #[serde(deserialize_with = "null_as_default")]
    pub name: String,
    #[serde(deserialize_with = "null_as_default")]
    pub album_artists: Vec<NamedItem>,
    #[serde(deserialize_with = "null_as_default")]
    pub artist_items: Vec<NamedItem>,
    #[serde(deserialize_with = "null_as_default")]
    pub genre_items: Vec<NamedItem>,
    pub image_tags: Option<ImageTags>,
    pub child_count: Option<u32>,
    pub run_time_ticks: Option<u64>,
    pub premiere_date: Option<String>,
    pub production_year: Option<i32>,
    pub date_created: Option<String>,
    pub user_data: Option<UserData>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct AlbumArtist {
    #[serde(deserialize_with = "null_as_default")]
    pub id: String,
    #[serde(deserialize_with = "null_as_default")]
    pub name: String,
    pub album_count: Option<u32>,
    pub song_count: Option<u32>,
    pub overview: Option<String>,
    pub image_tags: Option<ImageTags>,
    pub user_data: Option<UserData>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Playlist {
    #[serde(deserialize_with = "null_as_default")]
    pub id: String,
    #[serde(deserialize_with = "null_as_default")]
    pub name: String,
    pub overview: Option<String>,
    pub child_count: Option<u32>,
    pub run_time_ticks: Option<u64>,
    pub image_tags: Option<ImageTags>,
    pub date_created: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CreatedPlaylist {
    pub id: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Genre {
    #[serde(deserialize_with = "null_as_default")]
    pub id: String,
    #[serde(deserialize_with = "null_as_default")]
    pub name: String,
    pub song_count: Option<u32>,
    pub album_count: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct MusicFolder {
    #[serde(deserialize_with = "null_as_default")]
    pub id: String,
    #[serde(deserialize_with = "null_as_default")]
    pub name: String,
    pub collection_type: Option<String>,
}
