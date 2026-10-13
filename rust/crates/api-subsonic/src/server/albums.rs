use feishin_api::Result;
use feishin_core::model::{Album, AlbumDetail, AlbumListQuery, Page, Paging};

use super::SubsonicServer;
use super::counting::{MAX_ITEMS, count_by_probing};
use super::pages::{client_page, cloned_page, non_empty_search, server_page, wants_favorites_only};
use super::requests::SearchWindow;
use crate::album_list::{AlbumListType, album_list_params};
use crate::client::Params;
use crate::normalize;
use crate::sort::sort_albums;
use crate::types::{AlbumList2, GetAlbum, SubsonicAlbum};

enum AlbumSource<'query> {
    Search(&'query str),
    ArtistAlbums,
    Favorites,
    ServerList,
}

impl<'query> AlbumSource<'query> {
    fn for_query(query: &'query AlbumListQuery) -> Self {
        if let Some(term) = non_empty_search(query.search.as_ref()) {
            return Self::Search(term);
        }
        if !query.artist_ids.is_empty() {
            return Self::ArtistAlbums;
        }
        match wants_favorites_only(query.favorite) {
            true => Self::Favorites,
            false => Self::ServerList,
        }
    }
}

fn album_list_length(response: AlbumList2) -> usize {
    response
        .album_list2
        .and_then(|container| container.album)
        .map_or(0, |albums| albums.len())
}

impl SubsonicServer {
    pub(super) async fn fetch_album_page(
        &self,
        query: &AlbumListQuery,
        paging: Paging,
    ) -> Result<Page<Album>> {
        let folder = query.music_folder_id.as_deref();
        match AlbumSource::for_query(query) {
            AlbumSource::Search(term) => {
                let window = SearchWindow {
                    offset: paging.start_index,
                    count: paging.limit,
                };
                let albums = self.search_albums(term, window, folder).await?;
                Ok(server_page(self.normalize_albums(albums), paging, None))
            }
            AlbumSource::ArtistAlbums => {
                let albums = self.artist_albums(&query.artist_ids).await?;
                Ok(self.client_sorted_album_page(albums, query, paging))
            }
            AlbumSource::Favorites => {
                let starred = self.starred(folder).await?;
                let mut albums: Vec<&Album> = starred.albums.iter().collect();
                sort_albums(&mut albums, query.sort, query.order);
                Ok(cloned_page(client_page(albums, paging)))
            }
            AlbumSource::ServerList => self.server_album_page(query, paging).await,
        }
    }

    fn client_sorted_album_page(
        &self,
        albums: Vec<SubsonicAlbum>,
        query: &AlbumListQuery,
        paging: Paging,
    ) -> Page<Album> {
        let mut albums = self.normalize_albums(albums);
        sort_albums(&mut albums, query.sort, query.order);
        client_page(albums, paging)
    }

    async fn server_album_page(
        &self,
        query: &AlbumListQuery,
        paging: Paging,
    ) -> Result<Page<Album>> {
        let params = album_list_params(query, AlbumListType::for_sort(query.sort))
            .add("size", paging.limit)
            .add("offset", paging.start_index);
        let (response, total_count): (AlbumList2, _) =
            self.fetch_with_total("getAlbumList2.view", params).await?;
        let albums = response
            .album_list2
            .and_then(|container| container.album)
            .unwrap_or_default();
        Ok(server_page(
            self.normalize_albums(albums),
            paging,
            total_count,
        ))
    }

    pub(super) async fn count_albums(&self, query: &AlbumListQuery) -> Result<usize> {
        let folder = query.music_folder_id.as_deref();
        match AlbumSource::for_query(query) {
            AlbumSource::Search(term) => {
                count_by_probing(|offset, count| async move {
                    let window = SearchWindow { offset, count };
                    Ok(self.search_albums(term, window, folder).await?.len())
                })
                .await
            }
            AlbumSource::ArtistAlbums => Ok(self.artist_albums(&query.artist_ids).await?.len()),
            AlbumSource::Favorites => Ok(self.starred(folder).await?.albums.len()),
            AlbumSource::ServerList => self.count_server_album_list(query).await,
        }
    }

    async fn count_server_album_list(&self, query: &AlbumListQuery) -> Result<usize> {
        let base_params = album_list_params(query, AlbumListType::AlphabeticalByName);
        let mut start = 0;
        loop {
            let params = base_params
                .clone()
                .add("size", MAX_ITEMS)
                .add("offset", start);
            let (response, total_count): (AlbumList2, _) =
                self.fetch_with_total("getAlbumList2.view", params).await?;
            if let Some(total_count) = total_count {
                return Ok(total_count);
            }
            let received = album_list_length(response);
            start += received;
            if received != MAX_ITEMS {
                return Ok(start);
            }
        }
    }

    pub(super) async fn fetch_album_detail(&self, id: &str) -> Result<AlbumDetail> {
        let response: GetAlbum = self
            .fetch("getAlbum.view", Params::new().add("id", id))
            .await?;
        let (album, songs) = normalize::album(response.album, self.server_id());
        Ok(AlbumDetail { album, songs })
    }
}
