use feishin_core::model::Playlist;
use serde::Deserialize;

use crate::conversions::{non_empty, seconds_to_ms};
use crate::lenient::{optional_f64, optional_string, optional_u32, string, truthy};
use crate::subsonic::song::SubsonicSong;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct SubsonicPlaylist {
    #[serde(deserialize_with = "string")]
    pub id: String,
    #[serde(deserialize_with = "string")]
    pub name: String,
    #[serde(deserialize_with = "optional_string")]
    pub comment: Option<String>,
    #[serde(deserialize_with = "optional_string")]
    pub owner: Option<String>,
    #[serde(deserialize_with = "truthy")]
    pub public: bool,
    #[serde(deserialize_with = "optional_u32")]
    pub song_count: Option<u32>,
    #[serde(deserialize_with = "optional_f64")]
    pub duration: Option<f64>,
    #[serde(deserialize_with = "optional_string")]
    pub cover_art: Option<String>,
    #[serde(deserialize_with = "optional_string")]
    pub created: Option<String>,
    #[serde(deserialize_with = "optional_string")]
    pub changed: Option<String>,
    pub entry: Option<Vec<SubsonicSong>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct PlaylistResponse {
    pub playlist: Option<SubsonicPlaylist>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct PlaylistList {
    pub playlist: Vec<SubsonicPlaylist>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Playlists {
    pub playlists: Option<PlaylistList>,
}

pub(crate) fn normalize_playlist(playlist: SubsonicPlaylist, server_id: &str) -> Playlist {
    let cover_art_id =
        cache_busted_cover_art(non_empty(playlist.cover_art), playlist.changed.as_deref());
    Playlist {
        id: playlist.id,
        server_id: server_id.to_owned(),
        name: playlist.name,
        comment: non_empty(playlist.comment),
        owner: non_empty(playlist.owner),
        public: playlist.public,
        song_count: playlist.song_count.unwrap_or(0),
        duration_ms: seconds_to_ms(playlist.duration),
        cover_art_id,
        created_at: playlist.created,
        updated_at: playlist.changed,
    }
}

fn cache_busted_cover_art(cover_art: Option<String>, changed: Option<&str>) -> Option<String> {
    cover_art.map(|cover_art| match changed {
        Some(changed) if !changed.is_empty() => format!("{cover_art}&_={changed}"),
        _ => cover_art,
    })
}
