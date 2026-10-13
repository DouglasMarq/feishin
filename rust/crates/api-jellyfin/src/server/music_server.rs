use async_trait::async_trait;
use feishin_api::{ApiError, MusicServer, Result, StreamOptions};
use feishin_core::model::{
    Album, AlbumDetail, AlbumListQuery, Artist, ArtistDetail, ArtistListQuery, Genre, ItemKind,
    MusicFolder, Page, Paging, Playlist, SearchResults, ServerConfig, ServerPlayQueue, Song,
    SongListQuery,
};

use super::JellyfinServer;

#[async_trait]
impl MusicServer for JellyfinServer {
    fn config(&self) -> &ServerConfig {
        &self.config
    }

    async fn ping(&self) -> Result<()> {
        self.ping_system_info().await
    }

    async fn song_list(&self, query: &SongListQuery, paging: Paging) -> Result<Page<Song>> {
        self.song_page(query, paging).await
    }

    async fn song_count(&self, query: &SongListQuery) -> Result<usize> {
        self.count_songs(query).await
    }

    async fn song(&self, id: &str) -> Result<Song> {
        self.song_by_id(id).await
    }

    async fn random_songs(&self, count: usize, genre: Option<&str>) -> Result<Vec<Song>> {
        self.random_song_list(count, genre).await
    }

    async fn similar_songs(&self, song_id: &str, count: usize) -> Result<Vec<Song>> {
        self.similar_song_list(song_id, count).await
    }

    async fn album_list(&self, query: &AlbumListQuery, paging: Paging) -> Result<Page<Album>> {
        self.album_page(query, paging).await
    }

    async fn album_count(&self, query: &AlbumListQuery) -> Result<usize> {
        self.count_albums(query).await
    }

    async fn album(&self, id: &str) -> Result<AlbumDetail> {
        self.album_detail(id).await
    }

    async fn artist_list(&self, query: &ArtistListQuery, paging: Paging) -> Result<Page<Artist>> {
        self.artist_page(query, paging).await
    }

    async fn artist(&self, id: &str) -> Result<ArtistDetail> {
        self.artist_detail(id).await
    }

    async fn genre_list(&self) -> Result<Vec<Genre>> {
        self.all_genres().await
    }

    async fn music_folders(&self) -> Result<Vec<MusicFolder>> {
        self.music_libraries().await
    }

    async fn playlist_list(&self, paging: Paging) -> Result<Page<Playlist>> {
        self.playlist_page(paging).await
    }

    async fn playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        self.playlist_song_list(id).await
    }

    async fn playlist(&self, id: &str) -> Result<Playlist> {
        self.playlist_summary(id).await
    }

    async fn playlist_song_page(&self, id: &str, paging: Paging) -> Result<Page<Song>> {
        self.fetch_playlist_song_page(id, paging).await
    }

    async fn create_playlist(&self, name: &str, song_ids: &[String]) -> Result<Playlist> {
        self.create_audio_playlist(name, song_ids).await
    }

    async fn add_to_playlist(&self, playlist_id: &str, song_ids: &[String]) -> Result<()> {
        self.append_to_playlist(playlist_id, song_ids).await
    }

    async fn remove_from_playlist(
        &self,
        playlist_id: &str,
        playlist_positions: &[usize],
    ) -> Result<()> {
        self.remove_playlist_positions(playlist_id, playlist_positions)
            .await
    }

    async fn delete_playlist(&self, id: &str) -> Result<()> {
        self.delete_playlist_item(id).await
    }

    async fn search(&self, query: &str, limit: usize) -> Result<SearchResults> {
        self.search_library(query, limit).await
    }

    async fn set_favorite(&self, _kind: ItemKind, ids: &[String], favorite: bool) -> Result<()> {
        self.mark_favorites(ids, favorite).await
    }

    async fn set_rating(&self, _kind: ItemKind, _id: &str, _rating: u8) -> Result<()> {
        Err(ApiError::Unsupported("ratings"))
    }

    async fn scrobble(
        &self,
        song_id: &str,
        submission: bool,
        _timestamp_ms: Option<i64>,
    ) -> Result<()> {
        match submission {
            true => self.report_played_to_end(song_id).await,
            false => self.report_now_playing(song_id).await,
        }
    }

    async fn save_play_queue(
        &self,
        _song_ids: &[String],
        _current_song_id: Option<&str>,
        _position_ms: u64,
    ) -> Result<()> {
        Err(ApiError::Unsupported("server play queue"))
    }

    async fn play_queue(&self) -> Result<Option<ServerPlayQueue>> {
        Err(ApiError::Unsupported("server play queue"))
    }

    async fn lyrics(&self, song: &Song) -> Result<Option<String>> {
        self.song_lyrics(song).await
    }

    fn stream_url(&self, song: &Song, options: &StreamOptions) -> String {
        self.song_stream_url(&song.id, options)
    }

    fn cover_art_url(&self, cover_art_id: &str, edge_pixels: Option<u32>) -> String {
        self.primary_image_url(cover_art_id, edge_pixels)
    }
}
