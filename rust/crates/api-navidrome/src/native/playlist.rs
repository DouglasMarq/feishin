use feishin_core::model::Playlist;
use serde::Deserialize;

use crate::conversions::{non_empty, seconds_to_ms};
use crate::lenient::{optional_f64, optional_string, optional_u32, string, truthy};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct NativePlaylist {
    #[serde(deserialize_with = "string")]
    pub id: String,
    #[serde(deserialize_with = "string")]
    pub name: String,
    #[serde(deserialize_with = "optional_string")]
    pub comment: Option<String>,
    #[serde(deserialize_with = "optional_string")]
    pub owner_name: Option<String>,
    #[serde(deserialize_with = "truthy")]
    pub public: bool,
    #[serde(deserialize_with = "optional_u32")]
    pub song_count: Option<u32>,
    #[serde(deserialize_with = "optional_f64")]
    pub duration: Option<f64>,
    #[serde(deserialize_with = "optional_string")]
    pub created_at: Option<String>,
    #[serde(deserialize_with = "optional_string")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct NativeCreatedId {
    #[serde(deserialize_with = "string")]
    pub id: String,
}

pub(crate) fn normalize_playlist(playlist: NativePlaylist, server_id: &str) -> Playlist {
    let cover_art_id = format!(
        "{}&_={}",
        playlist.id,
        playlist.updated_at.as_deref().unwrap_or("")
    );
    Playlist {
        id: playlist.id,
        server_id: server_id.to_owned(),
        name: playlist.name,
        comment: non_empty(playlist.comment),
        owner: non_empty(playlist.owner_name),
        public: playlist.public,
        song_count: playlist.song_count.unwrap_or(0),
        duration_ms: seconds_to_ms(playlist.duration),
        cover_art_id: Some(cover_art_id),
        created_at: playlist.created_at,
        updated_at: playlist.updated_at,
    }
}
