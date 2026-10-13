use async_trait::async_trait;
use feishin_core::model::{
    Album, AlbumDetail, AlbumListQuery, Artist, ArtistDetail, ArtistListQuery, Genre, ItemKind,
    MusicFolder, Page, Paging, Playlist, SearchResults, ServerConfig, ServerPlayQueue, Song,
    SongListQuery,
};

use crate::credential::Credential;
use crate::error::Result;
use crate::stream_options::StreamOptions;

#[async_trait]
pub trait MusicServer: Send + Sync {
    fn config(&self) -> &ServerConfig;

    fn credential(&self) -> Option<Credential> {
        None
    }

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
    async fn playlist(&self, id: &str) -> Result<Playlist>;
    async fn playlist_song_page(&self, id: &str, paging: Paging) -> Result<Page<Song>>;
    async fn create_playlist(&self, name: &str, song_ids: &[String]) -> Result<Playlist>;
    async fn add_to_playlist(&self, playlist_id: &str, song_ids: &[String]) -> Result<()>;
    async fn remove_from_playlist(
        &self,
        playlist_id: &str,
        playlist_positions: &[usize],
    ) -> Result<()>;
    async fn delete_playlist(&self, id: &str) -> Result<()>;

    async fn search(&self, query: &str, limit: usize) -> Result<SearchResults>;

    async fn set_favorite(&self, kind: ItemKind, ids: &[String], favorite: bool) -> Result<()>;
    async fn set_rating(&self, kind: ItemKind, id: &str, rating_zero_clears: u8) -> Result<()>;

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

    async fn lyrics(&self, song: &Song) -> Result<Option<String>>;

    fn stream_url(&self, song: &Song, options: &StreamOptions) -> String;
    fn cover_art_url(&self, cover_art_id: &str, edge_pixels: Option<u32>) -> String;
}
