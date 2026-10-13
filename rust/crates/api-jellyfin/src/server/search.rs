use feishin_api::Result;
use feishin_core::model::{Album, Artist, SearchResults, Song, SortOrder};

use super::JellyfinServer;
use crate::dto;
use crate::item_fields::{
    ALBUM_ITEM_TYPE, ALBUM_LIST_FIELDS, ARTIST_LIST_FIELDS, SONG_FIELDS, SONG_ITEM_TYPE,
};
use crate::query_params::QueryParams;
use crate::sorting::{SORT_BY_NAME, sort_order};

const SONG_SEARCH_ORDER: &str = "Album,SortName";

impl JellyfinServer {
    pub(super) async fn search_library(&self, term: &str, limit: usize) -> Result<SearchResults> {
        if limit == 0 {
            return Ok(SearchResults {
                songs: Vec::new(),
                albums: Vec::new(),
                artists: Vec::new(),
            });
        }
        let albums = self.search_albums(term, limit).await?;
        let artists = self.search_artists(term, limit).await?;
        let songs = self.search_songs(term, limit).await?;
        Ok(SearchResults {
            songs,
            albums,
            artists,
        })
    }

    async fn search_albums(&self, term: &str, limit: usize) -> Result<Vec<Album>> {
        let params = QueryParams::default()
            .set("EnableTotalRecordCount", true)
            .fields(ALBUM_LIST_FIELDS)
            .set("ImageTypeLimit", 1)
            .set("IncludeItemTypes", ALBUM_ITEM_TYPE)
            .set("Limit", limit)
            .set("Recursive", true)
            .set("SearchTerm", term)
            .set("SortBy", SORT_BY_NAME)
            .set("SortOrder", sort_order(SortOrder::Asc))
            .set("StartIndex", 0);
        let list: dto::ItemList<dto::Album> = self.get(&self.items_path(), &params).await?;
        Ok(self.normalize_albums(list.items))
    }

    async fn search_artists(&self, term: &str, limit: usize) -> Result<Vec<Artist>> {
        let params = QueryParams::default()
            .set("EnableTotalRecordCount", true)
            .fields(ARTIST_LIST_FIELDS)
            .set("ImageTypeLimit", 1)
            .set("IncludeArtists", true)
            .set("Limit", limit)
            .set("Recursive", true)
            .set("SearchTerm", term)
            .set("StartIndex", 0)
            .set("UserId", &self.user_id);
        let list = self.fetch_album_artists(&params).await?;
        Ok(self.normalize_artists(list.items))
    }

    async fn search_songs(&self, term: &str, limit: usize) -> Result<Vec<Song>> {
        let params = QueryParams::default()
            .set("EnableTotalRecordCount", true)
            .fields(SONG_FIELDS)
            .set("IncludeItemTypes", SONG_ITEM_TYPE)
            .set("Limit", limit)
            .set("Recursive", true)
            .set("SearchTerm", term)
            .set("SortBy", SONG_SEARCH_ORDER)
            .set("SortOrder", sort_order(SortOrder::Asc))
            .set("StartIndex", 0)
            .set("UserId", &self.user_id);
        let list: dto::ItemList<dto::Song> = self.get(&self.items_path(), &params).await?;
        Ok(self.normalize_songs(list.items))
    }
}
