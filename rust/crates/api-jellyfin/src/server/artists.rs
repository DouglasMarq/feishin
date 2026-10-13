use feishin_api::Result;
use feishin_core::model::{
    Album, AlbumListQuery, AlbumSort, Artist, ArtistDetail, ArtistListQuery, Page, Paging, Song,
    SortOrder,
};

use super::JellyfinServer;
use crate::dto;
use crate::item_fields::{ARTIST_DETAIL_FIELDS, ARTIST_LIST_FIELDS, SONG_FIELDS, SONG_ITEM_TYPE};
use crate::normalize;
use crate::query_params::QueryParams;
use crate::sorting::{TOP_SONGS_ORDER, artist_sort, sort_order};

const ALBUM_ARTISTS_PATH: &str = "artists/albumArtists";

impl JellyfinServer {
    pub(super) fn normalize_artists(&self, items: Vec<dto::AlbumArtist>) -> Vec<Artist> {
        items
            .into_iter()
            .map(|item| normalize::artist(item, self.server_id()))
            .collect()
    }

    pub(super) async fn fetch_album_artists(
        &self,
        params: &QueryParams,
    ) -> Result<dto::ItemList<dto::AlbumArtist>> {
        self.get(ALBUM_ARTISTS_PATH, params).await
    }

    pub(super) async fn artist_page(
        &self,
        query: &ArtistListQuery,
        paging: Paging,
    ) -> Result<Page<Artist>> {
        let params = QueryParams::default()
            .fields(ARTIST_LIST_FIELDS)
            .set("ImageTypeLimit", 1)
            .set_optional("IsFavorite", query.favorite)
            .set("Limit", paging.limit)
            .set_optional("ParentId", query.music_folder_id.as_deref())
            .set("Recursive", true)
            .set_optional("SearchTerm", query.search.as_deref())
            .set("SortBy", artist_sort(query.sort))
            .set("SortOrder", sort_order(query.order))
            .set("StartIndex", paging.start_index)
            .set("UserId", &self.user_id);
        let list = self.fetch_album_artists(&params).await?;
        Ok(Page {
            total_count: list.total_record_count,
            items: self.normalize_artists(list.items),
            start_index: paging.start_index,
        })
    }

    pub(super) async fn artist_detail(&self, id: &str) -> Result<ArtistDetail> {
        let params = QueryParams::default().fields(ARTIST_DETAIL_FIELDS);
        let item: dto::AlbumArtist = self.get(&self.item_path(id), &params).await?;
        let albums = self.artist_albums(id).await?;
        let top_songs = self.artist_top_songs(id).await?;
        Ok(ArtistDetail {
            artist: normalize::artist(item, self.server_id()),
            albums,
            top_songs,
        })
    }

    async fn artist_albums(&self, artist_id: &str) -> Result<Vec<Album>> {
        let newest_first = AlbumListQuery {
            sort: AlbumSort::Year,
            order: SortOrder::Desc,
            artist_ids: vec![artist_id.to_owned()],
            ..AlbumListQuery::default()
        };
        Ok(self.fetch_albums(&newest_first, 0, None).await?.items)
    }

    async fn artist_top_songs(&self, artist_id: &str) -> Result<Vec<Song>> {
        let params = QueryParams::default()
            .set("ArtistIds", artist_id)
            .fields(SONG_FIELDS)
            .set("IncludeItemTypes", SONG_ITEM_TYPE)
            .set("Recursive", true)
            .set("SortBy", TOP_SONGS_ORDER)
            .set("SortOrder", sort_order(SortOrder::Desc))
            .set("UserId", &self.user_id);
        let list: dto::ItemList<dto::Song> = self.get(&self.items_path(), &params).await?;
        Ok(self.normalize_songs(list.items))
    }
}
