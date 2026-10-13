use feishin_api::{ApiError, Result};
use feishin_core::model::{Page, Paging, Playlist, Song};

use crate::NavidromeServer;
use crate::catalog::paging::{cloned_page, paginate};
use crate::features::Features;
use crate::native::{NativePlaylist, NativeSong, normalize_playlist};
use crate::query::{self, QueryParams, end_index};
use crate::subsonic;

impl NavidromeServer {
    pub(crate) async fn list_playlists(&self, paging: Paging) -> Result<Page<Playlist>> {
        self.native_or_subsonic(
            self.native_playlist_page(paging),
            self.subsonic_playlist_page(paging),
        )
        .await
    }

    pub(crate) async fn fetch_playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        let features = self.features().await;
        self.native_or_subsonic(
            self.native_playlist_songs(id, &features),
            self.subsonic_playlist_songs(id),
        )
        .await
    }

    pub(crate) async fn native_playlist(&self, id: &str) -> Result<Playlist> {
        let (playlist, _): (NativePlaylist, _) =
            self.native_get(&format!("playlist/{id}"), &[]).await?;
        Ok(normalize_playlist(playlist, self.server_id()))
    }

    pub(crate) async fn native_playlist_tracks(
        &self,
        id: &str,
        features: &Features,
    ) -> Result<Vec<NativeSong>> {
        let (tracks, _): (Vec<NativeSong>, _) = self
            .native_get(
                &format!("playlist/{id}/tracks"),
                &query::playlist_tracks(features),
            )
            .await?;
        Ok(tracks)
    }

    async fn native_playlist_page(&self, paging: Paging) -> Result<Page<Playlist>> {
        let start = paging.start_index;
        let params: QueryParams = vec![
            ("_start", start.to_string()),
            ("_end", end_index(start, paging.limit).to_string()),
            ("_sort", "name".into()),
            ("_order", "ASC".into()),
        ];
        let (playlists, total): (Vec<NativePlaylist>, _) =
            self.native_get("playlist", &params).await?;
        Ok(Page {
            items: playlists
                .into_iter()
                .map(|playlist| normalize_playlist(playlist, self.server_id()))
                .collect(),
            start_index: start,
            total_count: total,
        })
    }

    async fn subsonic_playlist_page(&self, paging: Paging) -> Result<Page<Playlist>> {
        let playlists = self.subsonic_playlists().await?;
        Ok(cloned_page(paginate(playlists.iter().collect(), paging)))
    }

    async fn native_playlist_songs(&self, id: &str, features: &Features) -> Result<Vec<Song>> {
        let tracks = self.native_playlist_tracks(id, features).await?;
        Ok(self.songs_from_native(tracks))
    }

    async fn subsonic_playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        let response: subsonic::PlaylistResponse = self
            .subsonic("getPlaylist", &[("id", id.to_owned())])
            .await?;
        let playlist = response.playlist.ok_or(ApiError::NotFound)?;
        Ok(self.songs_from_subsonic(playlist.entry.unwrap_or_default()))
    }
}
