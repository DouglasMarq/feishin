use feishin_api::Result;
use feishin_core::model::{Album, Artist, Song};
use serde::de::DeserializeOwned;

use super::SubsonicServer;
use crate::client::{Params, RequestEncoding};
use crate::extensions::Extensions;
use crate::normalize;
use crate::types::{Empty, SubsonicAlbum, SubsonicArtist, SubsonicSong};

impl SubsonicServer {
    fn request_encoding(&self) -> RequestEncoding {
        self.extensions
            .get()
            .map_or(RequestEncoding::QueryString, Extensions::request_encoding)
    }

    pub(super) async fn fetch<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        params: Params,
    ) -> Result<T> {
        self.fetch_with_total(endpoint, params)
            .await
            .map(|(response, _)| response)
    }

    pub(super) async fn fetch_with_total<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        params: Params,
    ) -> Result<(T, Option<usize>)> {
        let payload = self
            .client
            .call(endpoint, &params, self.request_encoding())
            .await?;
        let total_count = payload.total_count;
        Ok((payload.decode()?, total_count))
    }

    pub(super) async fn send(&self, endpoint: &str, params: Params) -> Result<()> {
        self.fetch::<Empty>(endpoint, params).await.map(drop)
    }

    pub(super) fn server_id(&self) -> &str {
        &self.config.id
    }

    pub(super) fn normalize_songs(&self, items: Vec<SubsonicSong>) -> Vec<Song> {
        items
            .into_iter()
            .map(|item| normalize::song(item, self.server_id()))
            .collect()
    }

    pub(super) fn normalize_albums(&self, items: Vec<SubsonicAlbum>) -> Vec<Album> {
        items
            .into_iter()
            .map(|item| normalize::album_without_songs(item, self.server_id()))
            .collect()
    }

    pub(super) fn normalize_artists(&self, items: Vec<SubsonicArtist>) -> Vec<Artist> {
        items
            .into_iter()
            .map(|item| normalize::artist(item, self.server_id()))
            .collect()
    }
}
