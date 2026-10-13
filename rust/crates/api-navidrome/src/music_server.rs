use async_trait::async_trait;
use feishin_api::{Credential, MusicServer, Result, StreamOptions};
use feishin_core::model::{
    Album, AlbumDetail, AlbumListQuery, Artist, ArtistDetail, ArtistListQuery, Genre, ItemKind,
    MusicFolder, Page, Paging, Playlist, SearchResults, ServerConfig, ServerPlayQueue, Song,
    SongListQuery,
};
use serde::de::IgnoredAny;

use crate::NavidromeServer;
use crate::features::OpenSubsonicExtension;
use crate::subsonic::{self, lyrics_text};

#[async_trait]
impl MusicServer for NavidromeServer {
    fn config(&self) -> &ServerConfig {
        &self.config
    }

    fn credential(&self) -> Option<Credential> {
        Some(NavidromeServer::credential(self))
    }

    async fn ping(&self) -> Result<()> {
        self.subsonic::<IgnoredAny>("ping", &[]).await.map(|_| ())
    }

    async fn song_list(&self, query: &SongListQuery, paging: Paging) -> Result<Page<Song>> {
        self.list_songs(query, paging).await
    }

    async fn song_count(&self, query: &SongListQuery) -> Result<usize> {
        self.count_songs(query).await
    }

    async fn song(&self, id: &str) -> Result<Song> {
        self.fetch_song(id).await
    }

    async fn random_songs(&self, count: usize, genre: Option<&str>) -> Result<Vec<Song>> {
        self.fetch_random_songs(count, genre).await
    }

    async fn similar_songs(&self, song_id: &str, count: usize) -> Result<Vec<Song>> {
        self.fetch_similar_songs(song_id, count).await
    }

    async fn album_list(&self, query: &AlbumListQuery, paging: Paging) -> Result<Page<Album>> {
        self.list_albums(query, paging).await
    }

    async fn album_count(&self, query: &AlbumListQuery) -> Result<usize> {
        self.count_albums(query).await
    }

    async fn album(&self, id: &str) -> Result<AlbumDetail> {
        self.fetch_album_detail(id).await
    }

    async fn artist_list(&self, query: &ArtistListQuery, paging: Paging) -> Result<Page<Artist>> {
        self.list_artists(query, paging).await
    }

    async fn artist(&self, id: &str) -> Result<ArtistDetail> {
        self.fetch_artist_detail(id).await
    }

    async fn genre_list(&self) -> Result<Vec<Genre>> {
        self.list_genres().await
    }

    async fn music_folders(&self) -> Result<Vec<MusicFolder>> {
        self.list_music_folders().await
    }

    async fn playlist_list(&self, paging: Paging) -> Result<Page<Playlist>> {
        self.list_playlists(paging).await
    }

    async fn playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        self.fetch_playlist_songs(id).await
    }

    async fn create_playlist(&self, name: &str, song_ids: &[String]) -> Result<Playlist> {
        self.create_playlist_with_songs(name, song_ids).await
    }

    async fn add_to_playlist(&self, playlist_id: &str, song_ids: &[String]) -> Result<()> {
        self.add_songs_to_playlist(playlist_id, song_ids).await
    }

    async fn remove_from_playlist(&self, playlist_id: &str, indexes: &[usize]) -> Result<()> {
        self.remove_playlist_entries(playlist_id, indexes).await
    }

    async fn delete_playlist(&self, id: &str) -> Result<()> {
        self.remove_playlist(id).await
    }

    async fn search(&self, query: &str, limit: usize) -> Result<SearchResults> {
        self.search_library(query, limit).await
    }

    async fn set_favorite(&self, kind: ItemKind, ids: &[String], favorite: bool) -> Result<()> {
        self.mark_favorite(kind, ids, favorite).await
    }

    async fn set_rating(&self, kind: ItemKind, id: &str, rating: u8) -> Result<()> {
        self.rate_item(kind, id, rating).await
    }

    async fn scrobble(
        &self,
        song_id: &str,
        submission: bool,
        timestamp_ms: Option<i64>,
    ) -> Result<()> {
        self.report_scrobble(song_id, submission, timestamp_ms)
            .await
    }

    async fn save_play_queue(
        &self,
        song_ids: &[String],
        current_song_id: Option<&str>,
        position_ms: u64,
    ) -> Result<()> {
        self.store_play_queue(song_ids, current_song_id, position_ms)
            .await
    }

    async fn play_queue(&self) -> Result<Option<ServerPlayQueue>> {
        self.fetch_play_queue().await
    }

    async fn lyrics(&self, song: &Song) -> Result<Option<String>> {
        let features = self.features().await;
        if !features.has_extension(OpenSubsonicExtension::SongLyrics) {
            return Ok(None);
        }
        let response: subsonic::LyricsList = self
            .subsonic("getLyricsBySongId", &[("id", song.id.clone())])
            .await?;
        Ok(lyrics_text(
            response.lyrics_list.unwrap_or_default().structured_lyrics,
        ))
    }

    fn stream_url(&self, song: &Song, options: &StreamOptions) -> String {
        self.build_stream_url(song, options)
    }

    fn cover_art_url(&self, cover_art_id: &str, size: Option<u32>) -> String {
        self.build_cover_art_url(cover_art_id, size)
    }
}
