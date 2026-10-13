use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::model::{ArtistRef, Song, SongId};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueueTrack {
    #[serde(alias = "id")]
    pub song_id: SongId,
    pub server_id: Arc<str>,
    pub title: Box<str>,
    #[serde(default)]
    pub artists: Arc<[ArtistRef]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub album: Option<Arc<str>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub album_id: Option<Arc<str>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_art_id: Option<Arc<str>>,
    #[serde(default)]
    pub duration_ms: u64,
}

impl QueueTrack {
    pub fn artist_names(&self) -> String {
        self.artists
            .iter()
            .map(|artist| artist.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl From<&Song> for QueueTrack {
    fn from(song: &Song) -> Self {
        Self {
            song_id: SongId::from(song.id.as_str()),
            server_id: Arc::from(song.server_id.as_str()),
            title: Box::from(song.title.as_str()),
            artists: Arc::from(song.artists.as_slice()),
            album: song.album.as_deref().map(Arc::from),
            album_id: song.album_id.as_deref().map(Arc::from),
            cover_art_id: song.cover_art_id.as_deref().map(Arc::from),
            duration_ms: song.duration_ms,
        }
    }
}
