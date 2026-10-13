//! The `MusicServer` trait every backend implements (mirrors `src/renderer/api/controller.ts`).
//!
//! Each backend crate also exposes, as inherent functions on its server type:
//! - `async fn login(config: ServerConfig, password: &str) -> Result<(Self, String)>`
//!   returning the server and an opaque credential string to persist.
//! - `fn with_credential(config: ServerConfig, credential: &str) -> Result<Self>`
//!   restoring a session from that credential without the password.

use async_trait::async_trait;
use feishin_core::model::{
    Album, AlbumDetail, AlbumListQuery, Artist, ArtistDetail, ArtistListQuery, Genre, ItemKind,
    MusicFolder, Page, Paging, Playlist, SearchResults, ServerConfig, ServerPlayQueue, Song,
    SongListQuery,
};

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("network error: {0}")]
    Network(String),
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("not found")]
    NotFound,
    #[error("server error {code}: {message}")]
    Server { code: i32, message: String },
    #[error("unexpected response: {0}")]
    Decode(String),
    #[error("not supported by this server: {0}")]
    Unsupported(&'static str),
}

pub type Result<T> = std::result::Result<T, ApiError>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StreamOptions {
    /// Ask the server to transcode down to this bitrate.
    pub max_bitrate_kbps: Option<u32>,
    /// Ask the server to transcode to this format (e.g. `opus`, `mp3`).
    pub format: Option<String>,
}

#[async_trait]
pub trait MusicServer: Send + Sync {
    fn config(&self) -> &ServerConfig;

    async fn ping(&self) -> Result<()>;

    async fn song_list(&self, query: &SongListQuery, paging: Paging) -> Result<Page<Song>>;
    async fn song_count(&self, query: &SongListQuery) -> Result<usize>;
    async fn song(&self, id: &str) -> Result<Song>;
    async fn random_songs(&self, count: usize, genre: Option<&str>) -> Result<Vec<Song>>;
    async fn similar_songs(&self, song_id: &str, count: usize) -> Result<Vec<Song>>;

    async fn album_list(&self, query: &AlbumListQuery, paging: Paging) -> Result<Page<Album>>;
    async fn album_count(&self, query: &AlbumListQuery) -> Result<usize>;
    async fn album(&self, id: &str) -> Result<AlbumDetail>;

    async fn artist_list(&self, query: &ArtistListQuery, paging: Paging) -> Result<Page<Artist>>;
    async fn artist(&self, id: &str) -> Result<ArtistDetail>;

    async fn genre_list(&self) -> Result<Vec<Genre>>;
    async fn music_folders(&self) -> Result<Vec<MusicFolder>>;

    async fn playlist_list(&self, paging: Paging) -> Result<Page<Playlist>>;
    async fn playlist_songs(&self, id: &str) -> Result<Vec<Song>>;
    async fn create_playlist(&self, name: &str, song_ids: &[String]) -> Result<Playlist>;
    async fn add_to_playlist(&self, playlist_id: &str, song_ids: &[String]) -> Result<()>;
    /// `indexes` are positions in the playlist as returned by `playlist_songs`.
    async fn remove_from_playlist(&self, playlist_id: &str, indexes: &[usize]) -> Result<()>;
    async fn delete_playlist(&self, id: &str) -> Result<()>;

    async fn search(&self, query: &str, limit: usize) -> Result<SearchResults>;

    async fn set_favorite(&self, kind: ItemKind, ids: &[String], favorite: bool) -> Result<()>;
    /// `rating` is 0-5, 0 clears it.
    async fn set_rating(&self, kind: ItemKind, id: &str, rating: u8) -> Result<()>;

    /// `submission = false` reports "now playing"; `true` records a play at `timestamp_ms` (Unix ms).
    async fn scrobble(
        &self,
        song_id: &str,
        submission: bool,
        timestamp_ms: Option<i64>,
    ) -> Result<()>;

    async fn save_play_queue(
        &self,
        song_ids: &[String],
        current_song_id: Option<&str>,
        position_ms: u64,
    ) -> Result<()>;
    async fn play_queue(&self) -> Result<Option<ServerPlayQueue>>;

    /// Lyrics stored on the server (OpenSubsonic `getLyricsBySongId`, Jellyfin lyrics), as LRC or plain text.
    async fn lyrics(&self, song: &Song) -> Result<Option<String>>;

    /// Authenticated URL that mpv can stream directly.
    fn stream_url(&self, song: &Song, options: &StreamOptions) -> String;
    /// Authenticated cover art URL; `size` is the requested edge in pixels.
    fn cover_art_url(&self, cover_art_id: &str, size: Option<u32>) -> String;
}
