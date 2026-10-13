use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use feishin_api::{Result, TtlCache};
use feishin_core::model::{Artist, Playlist};

use crate::NavidromeServer;
use crate::query::QueryParams;
use crate::subsonic;

const FALLBACK_LIST_TIME_TO_LIVE: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct MusicFolderKey(Option<String>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct AllPlaylists;

pub(crate) struct FallbackLists {
    artist_index: TtlCache<MusicFolderKey, Vec<Artist>>,
    playlists: TtlCache<AllPlaylists, Vec<Playlist>>,
}

impl FallbackLists {
    pub(crate) fn new() -> Self {
        Self {
            artist_index: TtlCache::new(FALLBACK_LIST_TIME_TO_LIVE),
            playlists: TtlCache::new(FALLBACK_LIST_TIME_TO_LIVE),
        }
    }
}

impl NavidromeServer {
    pub(crate) fn forget_fallback_lists(&self) {
        self.fallback_lists.artist_index.invalidate_all();
        self.fallback_lists.playlists.invalidate_all();
    }

    pub(crate) async fn changing_annotations<Output>(
        &self,
        change: impl Future<Output = Result<Output>>,
    ) -> Result<Output> {
        let outcome = change.await;
        self.fallback_lists.artist_index.invalidate_all();
        outcome
    }

    pub(crate) async fn changing_playlists<Output>(
        &self,
        change: impl Future<Output = Result<Output>>,
    ) -> Result<Output> {
        let outcome = change.await;
        self.fallback_lists.playlists.invalidate_all();
        outcome
    }

    pub(crate) async fn subsonic_artist_index(
        &self,
        music_folder_id: Option<&str>,
    ) -> Result<Arc<Vec<Artist>>> {
        let key = MusicFolderKey(music_folder_id.map(str::to_owned));
        self.fallback_lists
            .artist_index
            .get_or_fetch(&key, || self.fetch_subsonic_artist_index(music_folder_id))
            .await
    }

    async fn fetch_subsonic_artist_index(
        &self,
        music_folder_id: Option<&str>,
    ) -> Result<Vec<Artist>> {
        let params: QueryParams = music_folder_id
            .map(|folder| ("musicFolderId", folder.to_owned()))
            .into_iter()
            .collect();
        let response: subsonic::Artists = self.subsonic("getArtists", &params).await?;
        Ok(response
            .artists
            .unwrap_or_default()
            .index
            .into_iter()
            .flat_map(|index| index.artist)
            .map(|artist| subsonic::normalize_artist(artist, self.server_id()))
            .collect())
    }

    pub(crate) async fn subsonic_playlists(&self) -> Result<Arc<Vec<Playlist>>> {
        self.fallback_lists
            .playlists
            .get_or_fetch(&AllPlaylists, || self.fetch_subsonic_playlists())
            .await
    }

    async fn fetch_subsonic_playlists(&self) -> Result<Vec<Playlist>> {
        let response: subsonic::Playlists = self.subsonic("getPlaylists", &[]).await?;
        Ok(response
            .playlists
            .unwrap_or_default()
            .playlist
            .into_iter()
            .map(|playlist| subsonic::normalize_playlist(playlist, self.server_id()))
            .collect())
    }
}
