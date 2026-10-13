use feishin_api::Result;
use feishin_core::model::{Album, AlbumDetail, AlbumListQuery, Page, Paging, Song, SortOrder};

use super::JellyfinServer;
use crate::dto;
use crate::item_fields::{
    ALBUM_DETAIL_FIELDS, ALBUM_ITEM_TYPE, ALBUM_LIST_FIELDS, SONG_FIELDS, SONG_ITEM_TYPE,
};
use crate::normalize;
use crate::query_params::{QueryParams, comma_separated_ids, comma_separated_years};
use crate::sorting::{ALBUM_TRACK_ORDER, album_sort, sort_order};

impl JellyfinServer {
    pub(super) fn normalize_albums(&self, items: Vec<dto::Album>) -> Vec<Album> {
        items
            .into_iter()
            .map(|item| normalize::album(item, self.server_id()))
            .collect()
    }

    pub(super) async fn album_page(
        &self,
        query: &AlbumListQuery,
        paging: Paging,
    ) -> Result<Page<Album>> {
        self.fetch_albums(query, paging.start_index, Some(paging.limit))
            .await
    }

    pub(super) async fn count_albums(&self, query: &AlbumListQuery) -> Result<usize> {
        let page = self.fetch_albums(query, 0, Some(1)).await?;
        Ok(page.total_count.unwrap_or(page.items.len()))
    }

    pub(super) async fn album_detail(&self, id: &str) -> Result<AlbumDetail> {
        let detail_params = QueryParams::default().fields(ALBUM_DETAIL_FIELDS);
        let item: dto::Album = self.get(&self.item_path(id), &detail_params).await?;
        let songs = self.album_songs(id).await?;
        let mut album = normalize::album(item, self.server_id());
        if album.song_count == 0 {
            album.song_count = u32::try_from(songs.len()).unwrap_or(u32::MAX);
        }
        Ok(AlbumDetail { album, songs })
    }

    pub(super) async fn fetch_albums(
        &self,
        query: &AlbumListQuery,
        start: usize,
        limit: Option<usize>,
    ) -> Result<Page<Album>> {
        let params = album_params(query, start, limit);
        let list: dto::ItemList<dto::Album> = self.get(&self.items_path(), &params).await?;
        Ok(Page {
            total_count: list.total_record_count,
            items: self.normalize_albums(list.items),
            start_index: start,
        })
    }

    async fn album_songs(&self, album_id: &str) -> Result<Vec<Song>> {
        let params = QueryParams::default()
            .set("EnableUserData", true)
            .fields(SONG_FIELDS)
            .set("IncludeItemTypes", SONG_ITEM_TYPE)
            .set("ParentId", album_id)
            .set("Recursive", true)
            .set("SortBy", ALBUM_TRACK_ORDER)
            .set("SortOrder", sort_order(SortOrder::Asc))
            .set("StartIndex", 0)
            .set("UserId", &self.user_id);
        let list: dto::ItemList<dto::Song> = self.get(&self.items_path(), &params).await?;
        Ok(self.normalize_songs(list.items))
    }
}

fn album_params(query: &AlbumListQuery, start: usize, limit: Option<usize>) -> QueryParams {
    QueryParams::default()
        .set_optional("ArtistIds", comma_separated_ids(&query.artist_ids))
        .fields(ALBUM_LIST_FIELDS)
        .set_optional("GenreIds", comma_separated_ids(&query.genre_ids))
        .set("IncludeItemTypes", ALBUM_ITEM_TYPE)
        .set_optional("IsFavorite", query.favorite)
        .set_optional("Limit", limit)
        .set_optional("ParentId", query.music_folder_id.as_deref())
        .set("Recursive", true)
        .set_optional("SearchTerm", query.search.as_deref())
        .set("SortBy", album_sort(query.sort))
        .set("SortOrder", sort_order(query.order))
        .set("StartIndex", start)
        .set_optional(
            "Years",
            comma_separated_years(query.year_from, query.year_to),
        )
}
