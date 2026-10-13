use feishin_api::Result;
use feishin_core::model::{Page, Paging, Song};

use crate::NavidromeServer;
use crate::catalog::paging::paginate;
use crate::features::Features;
use crate::native::NativeSong;
use crate::query::{self, end_index_or_all};

impl NavidromeServer {
    pub(crate) async fn fetch_playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        let features = self.features().await;
        self.native_or_subsonic(
            self.native_playlist_songs(id, &features),
            self.subsonic_playlist_songs(id),
        )
        .await
    }

    pub(crate) async fn fetch_playlist_song_page(
        &self,
        id: &str,
        paging: Paging,
    ) -> Result<Page<Song>> {
        let features = self.features().await;
        self.native_or_subsonic(
            self.native_playlist_song_page(id, paging, &features),
            self.subsonic_playlist_song_page(id, paging),
        )
        .await
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

    async fn native_playlist_song_page(
        &self,
        id: &str,
        paging: Paging,
        features: &Features,
    ) -> Result<Page<Song>> {
        let start = paging.start_index;
        let params =
            query::playlist_track_page(start, end_index_or_all(start, paging.limit), features);
        let (tracks, total): (Vec<NativeSong>, _) = self
            .native_get(&format!("playlist/{id}/tracks"), &params)
            .await?;
        Ok(Page {
            items: self.songs_from_native(tracks),
            start_index: start,
            total_count: total,
        })
    }

    async fn native_playlist_songs(&self, id: &str, features: &Features) -> Result<Vec<Song>> {
        let tracks = self.native_playlist_tracks(id, features).await?;
        Ok(self.songs_from_native(tracks))
    }

    async fn subsonic_playlist_song_page(&self, id: &str, paging: Paging) -> Result<Page<Song>> {
        let songs = self.subsonic_playlist_songs(id).await?;
        Ok(paginate(songs, paging))
    }

    async fn subsonic_playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        let playlist = self.subsonic_playlist_detail(id).await?;
        Ok(self.songs_from_subsonic(playlist.entry.unwrap_or_default()))
    }
}
