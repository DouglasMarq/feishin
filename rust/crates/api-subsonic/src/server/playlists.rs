use feishin_api::{ApiError, Result};
use feishin_core::model::{Page, Paging, Playlist, Song};

use super::SubsonicServer;
use super::pages::client_page;
use crate::client::Params;
use crate::normalize;
use crate::types::{PlaylistResponse, Playlists, SubsonicSong};

impl SubsonicServer {
    pub(super) async fn fetch_playlist_page(&self, paging: Paging) -> Result<Page<Playlist>> {
        let response: Playlists = self.fetch("getPlaylists.view", Params::new()).await?;
        let playlists: Vec<Playlist> = response
            .playlists
            .and_then(|container| container.playlist)
            .unwrap_or_default()
            .into_iter()
            .map(|playlist| normalize::playlist(playlist, self.server_id()))
            .collect();
        Ok(client_page(playlists, paging))
    }

    async fn playlist_entries(&self, id: &str) -> Result<Vec<SubsonicSong>> {
        let response: PlaylistResponse = self
            .fetch("getPlaylist.view", Params::new().add("id", id))
            .await?;
        let playlist = response.playlist.ok_or(ApiError::NotFound)?;
        Ok(playlist.entry.unwrap_or_default())
    }

    pub(super) async fn fetch_playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        let entries = self.playlist_entries(id).await?;
        Ok(self.normalize_songs(entries))
    }

    pub(super) async fn create_playlist_with_songs(
        &self,
        name: &str,
        song_ids: &[String],
    ) -> Result<Playlist> {
        let params = Params::new().add("name", name).many("songId", song_ids);
        let response: PlaylistResponse = self.fetch("createPlaylist.view", params).await?;
        response
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
            .many("songIdToAdd", song_ids);
        self.send("updatePlaylist.view", params).await
    }

    pub(super) async fn remove_playlist_indexes(
        &self,
        playlist_id: &str,
        indexes: &[usize],
    ) -> Result<()> {
        let indexes: Vec<String> = indexes.iter().map(ToString::to_string).collect();
        let params = Params::new()
            .add("playlistId", playlist_id)
            .many("songIndexToRemove", &indexes);
        self.send("updatePlaylist.view", params).await
    }
}
