use feishin_api::{ApiError, Result};
use feishin_core::model::Playlist;
use reqwest::Method;
use serde::de::IgnoredAny;

use crate::NavidromeServer;
use crate::native::{CreatePlaylistRequest, NativeCreatedId, TrackIdsRequest};
use crate::query::QueryParams;
use crate::subsonic;

impl NavidromeServer {
    pub(crate) async fn create_playlist_with_songs(
        &self,
        name: &str,
        song_ids: &[String],
    ) -> Result<Playlist> {
        self.native_or_subsonic(
            self.native_create_playlist(name, song_ids),
            self.subsonic_create_playlist(name, song_ids),
        )
        .await
    }

    pub(crate) async fn add_songs_to_playlist(
        &self,
        playlist_id: &str,
        song_ids: &[String],
    ) -> Result<()> {
        if song_ids.is_empty() {
            return Ok(());
        }
        self.native_or_subsonic(
            self.native_add_tracks(playlist_id, song_ids),
            self.subsonic_add_tracks(playlist_id, song_ids),
        )
        .await
    }

    pub(crate) async fn remove_playlist_entries(
        &self,
        playlist_id: &str,
        indexes: &[usize],
    ) -> Result<()> {
        if indexes.is_empty() {
            return Ok(());
        }
        self.native_or_subsonic(
            self.native_remove_tracks(playlist_id, indexes),
            self.subsonic_remove_tracks(playlist_id, indexes),
        )
        .await
    }

    pub(crate) async fn remove_playlist(&self, id: &str) -> Result<()> {
        let native = async {
            self.native(Method::DELETE, &format!("playlist/{id}"), &[])
                .await
                .map(|_| ())
        };
        let subsonic = async {
            self.subsonic::<IgnoredAny>("deletePlaylist", &[("id", id.to_owned())])
                .await
                .map(|_| ())
        };
        self.native_or_subsonic(native, subsonic).await
    }

    async fn native_create_playlist(&self, name: &str, song_ids: &[String]) -> Result<Playlist> {
        let request = CreatePlaylistRequest { name, rules: None };
        let created: NativeCreatedId = self
            .native_with_body(Method::POST, "playlist", &request)
            .await?
            .json()?;
        if !song_ids.is_empty() {
            self.native_add_tracks(&created.id, song_ids).await?;
        }
        self.native_playlist(&created.id).await
    }

    async fn subsonic_create_playlist(&self, name: &str, song_ids: &[String]) -> Result<Playlist> {
        let features = self.features().await;
        let mut params: QueryParams = vec![("name", name.to_owned())];
        params.extend(song_ids.iter().map(|id| ("songId", id.clone())));
        let response: subsonic::PlaylistResponse = self
            .subsonic_large("createPlaylist", &params, &features)
            .await?;
        let playlist = response
            .playlist
            .ok_or_else(|| ApiError::Decode("createPlaylist returned no playlist".into()))?;
        Ok(subsonic::normalize_playlist(playlist, self.server_id()))
    }

    async fn native_add_tracks(&self, playlist_id: &str, song_ids: &[String]) -> Result<()> {
        self.native_with_body(
            Method::POST,
            &format!("playlist/{playlist_id}/tracks"),
            &TrackIdsRequest { ids: song_ids },
        )
        .await
        .map(|_| ())
    }

    async fn subsonic_add_tracks(&self, playlist_id: &str, song_ids: &[String]) -> Result<()> {
        let features = self.features().await;
        let mut params: QueryParams = vec![("playlistId", playlist_id.to_owned())];
        params.extend(song_ids.iter().map(|id| ("songIdToAdd", id.clone())));
        self.subsonic_large::<IgnoredAny>("updatePlaylist", &params, &features)
            .await
            .map(|_| ())
    }

    async fn native_remove_tracks(&self, playlist_id: &str, indexes: &[usize]) -> Result<()> {
        let features = self.features().await;
        let tracks = self.native_playlist_tracks(playlist_id, &features).await?;
        let params = indexes
            .iter()
            .map(|index| {
                tracks
                    .get(*index)
                    .map(|track| ("id", track.id.clone()))
                    .ok_or(ApiError::NotFound)
            })
            .collect::<Result<QueryParams>>()?;
        self.native(
            Method::DELETE,
            &format!("playlist/{playlist_id}/tracks"),
            &params,
        )
        .await
        .map(|_| ())
    }

    async fn subsonic_remove_tracks(&self, playlist_id: &str, indexes: &[usize]) -> Result<()> {
        let mut params: QueryParams = vec![("playlistId", playlist_id.to_owned())];
        params.extend(
            indexes
                .iter()
                .map(|index| ("songIndexToRemove", index.to_string())),
        );
        self.subsonic::<IgnoredAny>("updatePlaylist", &params)
            .await
            .map(|_| ())
    }
}
