use serde::{Deserialize, Serialize};

use super::album::Album;
use super::song::Song;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ArtistRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Artist {
    pub id: String,
    pub server_id: String,
    pub name: String,
    pub album_count: Option<u32>,
    pub song_count: Option<u32>,
    pub cover_art_id: Option<String>,
    pub favorite: bool,
    pub user_rating: Option<u8>,
    pub biography: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtistDetail {
    pub artist: Artist,
    pub albums: Vec<Album>,
    pub top_songs: Vec<Song>,
}
