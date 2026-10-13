use feishin_api::{ApiError, Result};
use feishin_core::model::{Album, AlbumDetail, AlbumListQuery, Page, Paging, SortOrder};

use crate::NavidromeServer;
use crate::catalog::SUBSONIC_MAX_ITEMS;
use crate::catalog::paging::subsonic_page;
use crate::features::Features;
use crate::http::missing_session_error;
use crate::native::{NativeAlbum, NativeSong, normalize_album};
use crate::query::{self, QueryParams, SubsonicAlbumListType, end_index};
use crate::subsonic;

const OLDEST_YEAR: &str = "0";
const NEWEST_YEAR: &str = "9999";

impl NavidromeServer {
    pub(crate) async fn list_albums(
        &self,
        query: &AlbumListQuery,
        paging: Paging,
    ) -> Result<Page<Album>> {
        let features = self.features().await;
        let start = paging.start_index;
        self.native_or_subsonic(
            self.native_album_page(query, start, end_index(start, paging.limit), &features),
            self.subsonic_album_page(query, paging),
        )
        .await
    }

    pub(crate) async fn count_albums(&self, query: &AlbumListQuery) -> Result<usize> {
        let features = self.features().await;
        let page = self.native_album_page(query, 0, 1, &features).await?;
        Ok(page.total_count.unwrap_or(0))
    }

    pub(crate) async fn fetch_album_detail(&self, id: &str) -> Result<AlbumDetail> {
        let features = self.features().await;
        self.native_or_subsonic(self.native_album(id, &features), self.subsonic_album(id))
            .await
    }

    pub(crate) async fn native_album_page(
        &self,
        query: &AlbumListQuery,
        start: usize,
        end: i64,
        features: &Features,
    ) -> Result<Page<Album>> {
        let params = query::album_list(query, start, end, features);
        let (albums, total): (Vec<NativeAlbum>, _) = self.native_get("album", &params).await?;
        Ok(Page {
            items: albums
                .into_iter()
                .map(|album| normalize_album(album, self.server_id()))
                .collect(),
            start_index: start,
            total_count: total,
        })
    }

    async fn subsonic_album_page(
        &self,
        query: &AlbumListQuery,
        paging: Paging,
    ) -> Result<Page<Album>> {
        let list_type = subsonic_album_list_type(query).ok_or_else(missing_session_error)?;
        let params = subsonic_album_list_params(query, paging, list_type);
        let result: subsonic::AlbumList2 = self.subsonic("getAlbumList2", &params).await?;
        let albums = result
            .album_list2
            .unwrap_or_default()
            .album
            .into_iter()
            .map(|album| subsonic::normalize_album(album, self.server_id()))
            .collect();
        Ok(subsonic_page(albums, paging.start_index))
    }

    async fn native_album(&self, id: &str, features: &Features) -> Result<AlbumDetail> {
        let (album, _): (NativeAlbum, _) = self.native_get(&format!("album/{id}"), &[]).await?;
        let (songs, _): (Vec<NativeSong>, _) = self
            .native_get("song", &query::album_songs(id, features))
            .await?;
        Ok(AlbumDetail {
            album: normalize_album(album, self.server_id()),
            songs: self.songs_from_native(songs),
        })
    }

    async fn subsonic_album(&self, id: &str) -> Result<AlbumDetail> {
        let response: subsonic::AlbumResponse =
            self.subsonic("getAlbum", &[("id", id.to_owned())]).await?;
        let mut album = response.album.ok_or(ApiError::NotFound)?;
        let songs = album.song.take().unwrap_or_default();
        Ok(AlbumDetail {
            album: subsonic::normalize_album(album, self.server_id()),
            songs: self.songs_from_subsonic(songs),
        })
    }
}

fn has_native_only_album_filters(query: &AlbumListQuery) -> bool {
    !query.artist_ids.is_empty()
        || !query.genre_ids.is_empty()
        || query
            .search
            .as_ref()
            .is_some_and(|search| !search.is_empty())
        || query.year_from.is_some()
        || query.year_to.is_some()
        || query.favorite == Some(false)
}

fn subsonic_album_list_type(query: &AlbumListQuery) -> Option<SubsonicAlbumListType> {
    if has_native_only_album_filters(query) {
        return None;
    }
    match query.favorite {
        Some(true) => Some(SubsonicAlbumListType::Starred),
        _ => SubsonicAlbumListType::for_sort(query.sort),
    }
}

fn subsonic_album_list_params(
    query: &AlbumListQuery,
    paging: Paging,
    list_type: SubsonicAlbumListType,
) -> QueryParams {
    let size = match paging.limit {
        0 => SUBSONIC_MAX_ITEMS,
        limit => limit.min(SUBSONIC_MAX_ITEMS),
    };
    let mut params: QueryParams = vec![
        ("type", list_type.query_value().to_owned()),
        ("size", size.to_string()),
        ("offset", paging.start_index.to_string()),
    ];
    if list_type == SubsonicAlbumListType::ByYear {
        params.extend(year_range_params(query.order));
    }
    params.extend(
        query
            .music_folder_id
            .clone()
            .map(|folder| ("musicFolderId", folder)),
    );
    params
}

fn year_range_params(order: SortOrder) -> [(&'static str, String); 2] {
    let (from_year, to_year) = match order {
        SortOrder::Asc => (OLDEST_YEAR, NEWEST_YEAR),
        SortOrder::Desc => (NEWEST_YEAR, OLDEST_YEAR),
    };
    [
        ("fromYear", from_year.to_owned()),
        ("toYear", to_year.to_owned()),
    ]
}
