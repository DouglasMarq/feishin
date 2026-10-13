use feishin_api::{ApiError, Result};
use futures_util::stream::{self, StreamExt};
use serde::de::DeserializeOwned;

use super::SubsonicServer;
use crate::client::Params;
use crate::types::{
    GetAlbum, GetArtist, Search3, SearchResult, SongsByGenre, SubsonicAlbum, SubsonicSong,
};

const CONCURRENT_REQUESTS: usize = 8;

#[derive(Debug, Clone, Copy, Default)]
pub struct SearchWindow {
    pub offset: usize,
    pub count: usize,
}

pub struct SearchRequest<'request> {
    pub query: &'request str,
    pub artists: SearchWindow,
    pub albums: SearchWindow,
    pub songs: SearchWindow,
    pub music_folder_id: Option<&'request str>,
}

fn search_params(request: &SearchRequest<'_>) -> Params {
    Params::new()
        .add("query", request.query)
        .add("artistCount", request.artists.count)
        .add("artistOffset", request.artists.offset)
        .add("albumCount", request.albums.count)
        .add("albumOffset", request.albums.offset)
        .add("songCount", request.songs.count)
        .add("songOffset", request.songs.offset)
        .add_optional("musicFolderId", request.music_folder_id)
}

fn keep_unless_rejected<T>(endpoint: &str, result: Result<T>) -> Option<Result<T>> {
    match result {
        Err(error @ (ApiError::Server { .. } | ApiError::NotFound)) => {
            tracing::debug!(endpoint, %error, "skipping item rejected by the server");
            None
        }
        other => Some(other),
    }
}

impl SubsonicServer {
    pub(super) async fn search3(&self, request: &SearchRequest<'_>) -> Result<SearchResult> {
        let response: Search3 = self.fetch("search3.view", search_params(request)).await?;
        Ok(response.search_result3.unwrap_or_default())
    }

    pub(super) async fn search_songs(
        &self,
        query: &str,
        window: SearchWindow,
        music_folder_id: Option<&str>,
    ) -> Result<Vec<SubsonicSong>> {
        let request = SearchRequest {
            query,
            artists: SearchWindow::default(),
            albums: SearchWindow::default(),
            songs: window,
            music_folder_id,
        };
        Ok(self.search3(&request).await?.song.unwrap_or_default())
    }

    pub(super) async fn search_albums(
        &self,
        query: &str,
        window: SearchWindow,
        music_folder_id: Option<&str>,
    ) -> Result<Vec<SubsonicAlbum>> {
        let request = SearchRequest {
            query,
            artists: SearchWindow::default(),
            albums: window,
            songs: SearchWindow::default(),
            music_folder_id,
        };
        Ok(self.search3(&request).await?.album.unwrap_or_default())
    }

    pub(super) async fn songs_by_genre(
        &self,
        genre: &str,
        window: SearchWindow,
        music_folder_id: Option<&str>,
    ) -> Result<Vec<SubsonicSong>> {
        let params = Params::new()
            .add("genre", genre)
            .add("count", window.count)
            .add("offset", window.offset)
            .add_optional("musicFolderId", music_folder_id);
        let response: SongsByGenre = self.fetch("getSongsByGenre.view", params).await?;
        Ok(response
            .songs_by_genre
            .and_then(|container| container.song)
            .unwrap_or_default())
    }

    async fn fetch_each_skipping_rejected<T: DeserializeOwned>(
        &self,
        endpoint: &'static str,
        ids: &[String],
    ) -> Result<Vec<T>> {
        let requests: Vec<_> = ids
            .iter()
            .map(|id| self.fetch::<T>(endpoint, Params::new().add("id", id)))
            .collect();
        let results: Vec<Result<T>> = stream::iter(requests)
            .buffered(CONCURRENT_REQUESTS)
            .collect()
            .await;
        results
            .into_iter()
            .filter_map(|result| keep_unless_rejected(endpoint, result))
            .collect()
    }

    pub(super) async fn artist_albums(&self, artist_ids: &[String]) -> Result<Vec<SubsonicAlbum>> {
        let artists: Vec<GetArtist> = self
            .fetch_each_skipping_rejected("getArtist.view", artist_ids)
            .await?;
        Ok(artists
            .into_iter()
            .flat_map(|response| response.artist.album.unwrap_or_default())
            .collect())
    }

    pub(super) async fn songs_of_albums_and_artists(
        &self,
        album_ids: &[String],
        artist_ids: &[String],
    ) -> Result<Vec<SubsonicSong>> {
        let mut ids = album_ids.to_vec();
        if !artist_ids.is_empty() {
            let artist_albums = self.artist_albums(artist_ids).await?;
            ids.extend(artist_albums.into_iter().map(|album| album.id));
        }
        let albums: Vec<GetAlbum> = self
            .fetch_each_skipping_rejected("getAlbum.view", &ids)
            .await?;
        Ok(albums
            .into_iter()
            .flat_map(|response| response.album.song.unwrap_or_default())
            .collect())
    }
}
