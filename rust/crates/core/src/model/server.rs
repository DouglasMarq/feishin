use serde::{Deserialize, Serialize};

use super::song::Song;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ServerType {
    Navidrome,
    Subsonic,
    Jellyfin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerConfig {
    pub id: String,
    pub name: String,
    pub server_type: ServerType,
    pub url: String,
    pub username: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerPlayQueue {
    pub songs: Vec<Song>,
    pub current_song_id: Option<String>,
    pub position_ms: u64,
}
