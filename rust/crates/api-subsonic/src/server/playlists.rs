use std::sync::Arc;

use feishin_api::{ApiError, Result};
use feishin_core::model::{Page, Paging, Playlist, Song};

use super::SubsonicServer;
use super::pages::{client_page, cloned_page};
use super::playlist_cache::CachedPlaylist;
use crate::client::Params;
use crate::normalize;
use crate::types::PlaylistResponse;

impl SubsonicServer {
    pub(super) async fn fetch_playlist_page(&self, paging: Paging) -> Result<Page<Playlist>> {
        let playlists = self.all_playlists().await?;
        Ok(cloned_page(client_page(playlists.iter().collect(), paging)))
    }

    async fn send_playlist_change(
        &self,
        endpoint: &str,
        params: Params,
        playlist_id: &str,
    ) -> Result<()> {
        let outcome = self.send(endpoint, params).await;
        self.list_caches.forget_playlists();
        self.playlist_cache.invalidate(playlist_id).await;
        outcome
    }

    async fn cached_playlist(&self, id: &str) -> Result<Arc<CachedPlaylist>> {
        self.playlist_cache
            .get_or_load(id, self.load_playlist(id))
            .await
    }

    async fn load_playlist(&self, id: &str) -> Result<CachedPlaylist> {
        let response: PlaylistResponse = self
            .fetch("getPlaylist.view", Params::new().add("id", id))
            .await?;
        let mut playlist = response.playlist.ok_or(ApiError::NotFound)?;
        let entries = playlist.entry.take().unwrap_or_default();
        Ok(CachedPlaylist::new(
            normalize::playlist(playlist, self.server_id()),
            self.normalize_songs(entries),
        ))
    }

    pub(super) async fn fetch_playlist(&self, id: &str) -> Result<Playlist> {
        let cached = self.cached_playlist(id).await?;
        Ok(cached.playlist.clone())
    }

    pub(super) async fn fetch_playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        let cached = self.cached_playlist(id).await?;
        Ok(cached.songs().to_vec())
    }

    pub(super) async fn fetch_playlist_song_page(
        &self,
        id: &str,
        paging: Paging,
    ) -> Result<Page<Song>> {
        let cached = self.cached_playlist(id).await?;
        Ok(cached.song_page(paging))
    }

    pub(super) async fn create_playlist_with_songs(
        &self,
        name: &str,
        song_ids: &[String],
    ) -> Result<Playlist> {
        let params = Params::new().add("name", name).add_each("songId", song_ids);
        let created: Result<PlaylistResponse> = self.fetch("createPlaylist.view", params).await;
        self.list_caches.forget_playlists();
        created?
            .playlist
            .map(|playlist| normalize::playlist(playlist, self.server_id()))
            .ok_or_else(|| ApiError::Decode("createPlaylist returned no playlist".into()))
    }

    pub(super) async fn append_to_playlist(
        &self,
        playlist_id: &str,
        song_ids: &[String],
    ) -> Result<()> {
        let params = Params::new()
            .add("playlistId", playlist_id)
            .add_each("songIdToAdd", song_ids);
        self.send_playlist_change("updatePlaylist.view", params, playlist_id)
            .await
    }

    pub(super) async fn remove_playlist_indexes(
        &self,
        playlist_id: &str,
        indexes: &[usize],
    ) -> Result<()> {
        let indexes: Vec<String> = indexes.iter().map(ToString::to_string).collect();
        let params = Params::new()
            .add("playlistId", playlist_id)
            .add_each("songIndexToRemove", &indexes);
        self.send_playlist_change("updatePlaylist.view", params, playlist_id)
            .await
    }

    pub(super) async fn delete_playlist_by_id(&self, id: &str) -> Result<()> {
        self.send_playlist_change("deletePlaylist.view", Params::new().add("id", id), id)
            .await
    }
}
