use serde::{Deserialize, Serialize};

use super::artist::ArtistRef;
use super::song::Song;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Album {
    pub id: String,
    pub server_id: String,
    pub name: String,
    pub album_artists: Vec<ArtistRef>,
    pub artists: Vec<ArtistRef>,
    pub year: Option<i32>,
    pub genres: Vec<String>,
    pub song_count: u32,
    pub duration_ms: u64,
    pub cover_art_id: Option<String>,
    pub play_count: u32,
    pub user_rating: Option<u8>,
    pub favorite: bool,
    pub created_at: Option<String>,
    pub last_played_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlbumDetail {
    pub album: Album,
    pub songs: Vec<Song>,
}
