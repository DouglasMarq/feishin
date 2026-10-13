//! Server-agnostic library types. Every backend maps its responses into these.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ServerType {
    Navidrome,
    Subsonic,
    Jellyfin,
}

/// A configured server. Secrets are not stored here; see `feishin_storage::credentials`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerConfig {
    pub id: String,
    pub name: String,
    pub server_type: ServerType,
    /// Base URL without trailing slash, e.g. `https://music.example.com`.
    pub url: String,
    pub username: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtistRef {
    pub id: String,
    pub name: String,
}

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
    /// 0-5, `None` when unrated.
    pub user_rating: Option<u8>,
    pub favorite: bool,
    pub cover_art_id: Option<String>,
    pub replay_gain: Option<ReplayGain>,
    pub path: Option<String>,
    pub size_bytes: Option<u64>,
    pub container: Option<String>,
    /// RFC 3339 timestamps as returned by the server.
    pub created_at: Option<String>,
    pub last_played_at: Option<String>,
}

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub server_id: String,
    pub name: String,
    pub comment: Option<String>,
    pub owner: Option<String>,
    pub public: bool,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResults {
    pub songs: Vec<Song>,
    pub albums: Vec<Album>,
    pub artists: Vec<Artist>,
}

/// Queue stored on the server (Subsonic `savePlayQueue` / `getPlayQueue`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerPlayQueue {
    pub songs: Vec<Song>,
    pub current_song_id: Option<String>,
    pub position_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub start_index: usize,
    /// Total matching items when the server reports it.
    pub total_count: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Paging {
    pub start_index: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SortOrder {
    #[default]
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SongSort {
    #[default]
    Name,
    Album,
    AlbumArtist,
    Artist,
    Duration,
    Year,
    PlayCount,
    Rating,
    Favorited,
    RecentlyAdded,
    RecentlyPlayed,
    Random,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SongListQuery {
    pub sort: SongSort,
    pub order: SortOrder,
    pub search: Option<String>,
    pub favorite: Option<bool>,
    pub genre_ids: Vec<String>,
    pub artist_ids: Vec<String>,
    pub album_ids: Vec<String>,
    pub year_from: Option<i32>,
    pub year_to: Option<i32>,
    pub music_folder_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AlbumSort {
    #[default]
    Name,
    AlbumArtist,
    Year,
    PlayCount,
    Rating,
    Favorited,
    RecentlyAdded,
    RecentlyPlayed,
    SongCount,
    Duration,
    Random,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AlbumListQuery {
    pub sort: AlbumSort,
    pub order: SortOrder,
    pub search: Option<String>,
    pub favorite: Option<bool>,
    pub genre_ids: Vec<String>,
    pub artist_ids: Vec<String>,
    pub year_from: Option<i32>,
    pub year_to: Option<i32>,
    pub music_folder_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ArtistSort {
    #[default]
    Name,
    AlbumCount,
    SongCount,
    PlayCount,
    Rating,
    Favorited,
    Random,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ArtistListQuery {
    pub sort: ArtistSort,
    pub order: SortOrder,
    pub search: Option<String>,
    pub favorite: Option<bool>,
    pub music_folder_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemKind {
    Song,
    Album,
    Artist,
    Playlist,
}
