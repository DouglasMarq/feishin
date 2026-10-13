use feishin_api::Result;
use feishin_core::model::{Page, Paging, Playlist, Song};

use super::JellyfinServer;
use crate::dto;
use crate::item_fields::{PLAYLIST_DETAIL_FIELDS, PLAYLIST_LIST_FIELDS, SONG_ITEM_TYPE};
use crate::normalize;
use crate::query_params::QueryParams;

impl JellyfinServer {
    pub(super) async fn playlist_song_list(&self, id: &str) -> Result<Vec<Song>> {
        let items = self.playlist_items(id, PLAYLIST_DETAIL_FIELDS).await?;
        Ok(self.normalize_songs(items))
    }

    pub(super) async fn playlist_summary(&self, id: &str) -> Result<Playlist> {
        let params = QueryParams::default()
            .fields(PLAYLIST_LIST_FIELDS)
            .set("Ids", id);
        let item: dto::Playlist = self.get(&self.item_path(id), &params).await?;
        Ok(normalize::playlist(item, self.server_id()))
    }

    pub(super) async fn fetch_playlist_song_page(
        &self,
        id: &str,
        paging: Paging,
    ) -> Result<Page<Song>> {
        let params = self
            .playlist_item_params(PLAYLIST_DETAIL_FIELDS)
            .set("StartIndex", paging.start_index)
            .set("Limit", paging.limit);
        let list: dto::ItemList<dto::Song> = self.get(&playlist_items_path(id), &params).await?;
        Ok(Page {
            total_count: list.total_record_count,
            items: self.normalize_songs(list.items),
            start_index: paging.start_index,
        })
    }

    pub(super) async fn playlist_items(&self, id: &str, fields: &[&str]) -> Result<Vec<dto::Song>> {
        let params = self.playlist_item_params(fields);
        let list: dto::ItemList<dto::Song> = self.get(&playlist_items_path(id), &params).await?;
        Ok(list.items)
    }

    fn playlist_item_params(&self, fields: &[&str]) -> QueryParams {
        QueryParams::default()
            .fields(fields)
            .set("IncludeItemTypes", SONG_ITEM_TYPE)
            .set("UserId", &self.user_id)
    }
}

pub(super) fn playlist_items_path(playlist_id: &str) -> String {
    format!("playlists/{playlist_id}/items")
}
