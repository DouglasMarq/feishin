use feishin_api::Result;
use feishin_core::model::{Folder, FolderId, Page, Paging, Song, SortOrder};

use super::JellyfinServer;
use crate::dto;
use crate::item_fields::{NO_FIELDS, SONG_FIELDS, SONG_ITEM_TYPE};
use crate::normalize;
use crate::query_params::QueryParams;
use crate::sorting::{ALBUM_TRACK_ORDER, SORT_BY_NAME, sort_order};

fn empty_page(paging: Paging) -> Page<Song> {
    Page {
        items: Vec::new(),
        start_index: paging.start_index,
        total_count: Some(0),
    }
}

fn subfolder_params(parent_id: &str) -> QueryParams {
    QueryParams::default()
        .fields(NO_FIELDS)
        .set("IsFolder", true)
        .set("ParentId", parent_id)
        .set("SortBy", SORT_BY_NAME)
        .set("SortOrder", sort_order(SortOrder::Asc))
}

fn folder_song_params(parent_id: &str, paging: Paging) -> QueryParams {
    QueryParams::default()
        .set("EnableTotalRecordCount", true)
        .fields(SONG_FIELDS)
        .set("IncludeItemTypes", SONG_ITEM_TYPE)
        .set("Limit", paging.limit)
        .set("ParentId", parent_id)
        .set("SortBy", ALBUM_TRACK_ORDER)
        .set("SortOrder", sort_order(SortOrder::Asc))
        .set("StartIndex", paging.start_index)
}

impl JellyfinServer {
    pub(super) async fn fetch_subfolders(&self, folder: &FolderId) -> Result<Vec<Folder>> {
        match folder {
            FolderId::Root => self.library_folders().await,
            FolderId::Directory(parent_id) => self.child_folders(parent_id).await,
        }
    }

    pub(super) async fn fetch_folder_song_page(
        &self,
        folder: &FolderId,
        paging: Paging,
    ) -> Result<Page<Song>> {
        let FolderId::Directory(parent_id) = folder else {
            return Ok(empty_page(paging));
        };
        let params = folder_song_params(parent_id, paging);
        let list: dto::ItemList<dto::Song> = self.get(&self.items_path(), &params).await?;
        let total = list.total_record_count;
        Ok(Page {
            items: self.normalize_songs(list.items),
            start_index: paging.start_index,
            total_count: total,
        })
    }

    async fn library_folders(&self) -> Result<Vec<Folder>> {
        let libraries = self.music_libraries().await?;
        Ok(libraries
            .into_iter()
            .map(normalize::library_folder)
            .collect())
    }

    async fn child_folders(&self, parent_id: &str) -> Result<Vec<Folder>> {
        let params = subfolder_params(parent_id);
        let list: dto::ItemList<dto::FolderItem> = self.get(&self.items_path(), &params).await?;
        Ok(list.items.into_iter().map(normalize::folder).collect())
    }
}
