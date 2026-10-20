use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub server_id: String,
    pub name: String,
    pub comment: Option<String>,
    pub owner: Option<String>,
    #[serde(rename = "public")]
    pub is_public: bool,
    pub song_count: u32,
    pub duration_ms: u64,
    pub cover_art_id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Genre {
    pub id: String,
    pub name: String,
    pub song_count: Option<u32>,
    pub album_count: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicFolder {
    pub id: String,
    pub name: String,
}
