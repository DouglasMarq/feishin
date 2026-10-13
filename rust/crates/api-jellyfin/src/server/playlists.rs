use feishin_api::{ApiError, Result};
use feishin_core::model::{Page, Paging, Playlist, SortOrder};
use reqwest::Method;

use super::playlist_items::playlist_items_path;
use super::{JellyfinServer, MAX_IDS_PER_QUERY_STRING};
use crate::dto;
use crate::item_fields::{NO_FIELDS, PLAYLIST_ITEM_TYPE, PLAYLIST_LIST_FIELDS};
use crate::normalize;
use crate::query_params::QueryParams;
use crate::sorting::{SORT_BY_NAME, sort_order};
use crate::transport::{decode, send};

const PLAYLIST_MEDIA_TYPES: &str = "Audio, Unknown";
const AUDIO_MEDIA_TYPE: &str = "Audio";

impl JellyfinServer {
    pub(super) async fn playlist_page(&self, paging: Paging) -> Result<Page<Playlist>> {
        let params = QueryParams::default()
            .fields(PLAYLIST_LIST_FIELDS)
            .set("IncludeItemTypes", PLAYLIST_ITEM_TYPE)
            .set("Limit", paging.limit)
            .set("MediaTypes", PLAYLIST_MEDIA_TYPES)
            .set("Recursive", true)
            .set("SortBy", SORT_BY_NAME)
            .set("SortOrder", sort_order(SortOrder::Asc))
            .set("StartIndex", paging.start_index);
        let list: dto::ItemList<dto::Playlist> = self.get(&self.items_path(), &params).await?;
        Ok(Page {
            total_count: list.total_record_count,
            items: list
                .items
                .into_iter()
                .map(|item| normalize::playlist(item, self.server_id()))
                .collect(),
            start_index: paging.start_index,
        })
    }

    pub(super) async fn create_audio_playlist(
        &self,
        name: &str,
        song_ids: &[String],
    ) -> Result<Playlist> {
        let new_playlist = dto::NewPlaylist {
            media_type: AUDIO_MEDIA_TYPE,
            name,
            user_id: &self.user_id,
        };
        let response = send(self.request(Method::POST, "playlists").json(&new_playlist)).await?;
        let created: dto::CreatedPlaylist = decode(response).await?;
        self.append_to_playlist(&created.id, song_ids).await?;
        self.playlist_summary(&created.id).await
    }

    pub(super) async fn append_to_playlist(
        &self,
        playlist_id: &str,
        song_ids: &[String],
    ) -> Result<()> {
        let path = playlist_items_path(playlist_id);
        for song_batch in song_ids.chunks(MAX_IDS_PER_QUERY_STRING) {
            let params = QueryParams::default()
                .set("Ids", song_batch.join(","))
                .set("UserId", &self.user_id);
            let request = self.request(Method::POST, &path).query(params.pairs());
            self.execute(request).await?;
        }
        Ok(())
    }

    pub(super) async fn remove_playlist_positions(
        &self,
        playlist_id: &str,
        positions: &[usize],
    ) -> Result<()> {
        if positions.is_empty() {
            return Ok(());
        }
        let items = self.playlist_items(playlist_id, NO_FIELDS).await?;
        let entry_ids = entry_ids_at(&items, positions)?;
        let path = playlist_items_path(playlist_id);
        for entry_batch in entry_ids.chunks(MAX_IDS_PER_QUERY_STRING) {
            let params = QueryParams::default().set("EntryIds", entry_batch.join(","));
            let request = self.request(Method::DELETE, &path).query(params.pairs());
            self.execute(request).await?;
        }
        Ok(())
    }

    pub(super) async fn delete_playlist_item(&self, id: &str) -> Result<()> {
        self.execute(self.request(Method::DELETE, &format!("items/{id}")))
            .await
    }
}

fn entry_ids_at(items: &[dto::Song], positions: &[usize]) -> Result<Vec<String>> {
    positions
        .iter()
        .map(|position| {
            items
                .get(*position)
                .and_then(|item| item.playlist_item_id.clone())
                .ok_or(ApiError::NotFound)
        })
        .collect()
}
