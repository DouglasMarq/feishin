use serde::{Deserialize, Deserializer, Serialize};

fn nullable<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuthResponse {
    pub access_token: String,
    pub user: AuthUser,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuthUser {
    pub id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase", bound(deserialize = "T: Deserialize<'de>"))]
pub struct ItemList<T> {
    #[serde(default = "Vec::new", deserialize_with = "nullable")]
    pub items: Vec<T>,
    #[serde(default)]
    pub total_record_count: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct NamedItem {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    #[serde(deserialize_with = "nullable")]
    pub name: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ImageTags {
    pub primary: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct UserData {
    #[serde(deserialize_with = "nullable")]
    pub is_favorite: bool,
    #[serde(deserialize_with = "nullable")]
    pub play_count: u32,
    pub last_played_date: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct MediaStream {
    #[serde(rename = "Type", deserialize_with = "nullable")]
    pub kind: String,
    pub bit_rate: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct MediaSource {
    pub container: Option<String>,
    pub path: Option<String>,
    pub size: Option<u64>,
    #[serde(deserialize_with = "nullable")]
    pub media_streams: Vec<MediaStream>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Song {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    #[serde(deserialize_with = "nullable")]
    pub name: String,
    pub album: Option<String>,
    pub album_id: Option<String>,
    pub album_primary_image_tag: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub album_artists: Vec<NamedItem>,
    #[serde(deserialize_with = "nullable")]
    pub artist_items: Vec<NamedItem>,
    #[serde(deserialize_with = "nullable")]
    pub genre_items: Vec<NamedItem>,
    pub image_tags: Option<ImageTags>,
    pub index_number: Option<u32>,
    pub parent_index_number: Option<u32>,
    pub run_time_ticks: Option<u64>,
    pub premiere_date: Option<String>,
    pub production_year: Option<i32>,
    pub date_created: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub media_sources: Vec<MediaSource>,
    pub user_data: Option<UserData>,
    pub normalization_gain: Option<f32>,
    pub album_normalization_gain: Option<f32>,
    #[serde(rename = "LUFS")]
    pub lufs: Option<f32>,
    pub playlist_item_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Album {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    #[serde(deserialize_with = "nullable")]
    pub name: String,
    #[serde(deserialize_with = "nullable")]
    pub album_artists: Vec<NamedItem>,
    #[serde(deserialize_with = "nullable")]
    pub artist_items: Vec<NamedItem>,
    #[serde(deserialize_with = "nullable")]
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
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    #[serde(deserialize_with = "nullable")]
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
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    #[serde(deserialize_with = "nullable")]
    pub name: String,
    pub overview: Option<String>,
    pub child_count: Option<u32>,
    pub run_time_ticks: Option<u64>,
    pub image_tags: Option<ImageTags>,
    pub date_created: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Genre {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    #[serde(deserialize_with = "nullable")]
    pub name: String,
    pub song_count: Option<u32>,
    pub album_count: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct MusicFolder {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    #[serde(deserialize_with = "nullable")]
    pub name: String,
    pub collection_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CreatedPlaylist {
    pub id: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Lyrics {
    #[serde(deserialize_with = "nullable")]
    pub lyrics: Vec<LyricLine>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct LyricLine {
    pub start: Option<u64>,
    #[serde(deserialize_with = "nullable")]
    pub text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlaybackReport<'a> {
    pub item_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_paused: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_ticks: Option<u64>,
}
