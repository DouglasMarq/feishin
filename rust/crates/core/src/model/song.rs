use serde::{Deserialize, Serialize};

use super::artist::ArtistRef;

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct ReplayGain {
    pub track_gain_db: Option<f32>,
    pub track_peak: Option<f32>,
    pub album_gain_db: Option<f32>,
    pub album_peak: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Song {
    pub id: String,
    pub server_id: String,
    pub title: String,
    pub artists: Vec<ArtistRef>,
    pub album_artists: Vec<ArtistRef>,
    pub album: Option<String>,
    pub album_id: Option<String>,
    pub duration_ms: u64,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub year: Option<i32>,
    pub genres: Vec<String>,
    pub bitrate_kbps: Option<u32>,
    pub play_count: u32,
    pub user_rating: Option<u8>,
    pub favorite: bool,
    pub cover_art_id: Option<String>,
    pub replay_gain: Option<ReplayGain>,
    pub path: Option<String>,
    pub size_bytes: Option<u64>,
    pub container: Option<String>,
    pub created_at: Option<String>,
    pub last_played_at: Option<String>,
}
