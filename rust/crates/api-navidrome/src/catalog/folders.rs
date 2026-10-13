use std::sync::Arc;

use feishin_api::Result;
use feishin_core::model::{Folder, FolderId, Page, Paging, Song};

use crate::NavidromeServer;
use crate::subsonic::{self, DirectoryResponse, FolderListing, IndexesResponse};

fn song_page(songs: &[Song], paging: Paging) -> Page<Song> {
    Page {
        items: songs
            .iter()
            .skip(paging.start_index)
            .take(paging.limit)
            .cloned()
            .collect(),
        start_index: paging.start_index,
        total_count: Some(songs.len()),
    }
}

impl NavidromeServer {
    pub(crate) async fn list_subfolders(&self, folder: &FolderId) -> Result<Vec<Folder>> {
        Ok(self.folder_listing(folder).await?.folders.clone())
    }

    pub(crate) async fn list_folder_songs(
        &self,
        folder: &FolderId,
        paging: Paging,
    ) -> Result<Page<Song>> {
        let listing = self.folder_listing(folder).await?;
        Ok(song_page(&listing.songs, paging))
    }

    pub(crate) fn forget_folder_listing(&self) {
        self.latest_folder.invalidate();
    }

    async fn folder_listing(&self, folder: &FolderId) -> Result<Arc<FolderListing>> {
        self.latest_folder
            .get_or_fetch(folder, || self.fetch_folder_listing(folder))
            .await
    }

    async fn fetch_folder_listing(&self, folder: &FolderId) -> Result<FolderListing> {
        match folder {
            FolderId::Root => self.fetch_indexes().await,
            FolderId::Directory(id) => self.fetch_music_directory(id).await,
        }
    }

    async fn fetch_indexes(&self) -> Result<FolderListing> {
        let response: IndexesResponse = self.subsonic("getIndexes", &[]).await?;
        let indexes = response.indexes.unwrap_or_default();
        let mut listing = subsonic::folder_listing(indexes.child, self.server_id());
        listing.folders = indexes
            .index
            .into_iter()
            .flat_map(|index| index.artist)
            .map(subsonic::indexed_folder)
            .chain(listing.folders)
            .collect();
        Ok(listing)
    }

    async fn fetch_music_directory(&self, id: &str) -> Result<FolderListing> {
        let params = [("id", id.to_owned())];
        let response: DirectoryResponse = self.subsonic("getMusicDirectory", &params).await?;
        let children = response
            .directory
            .map(|directory| directory.child)
            .unwrap_or_default();
        Ok(subsonic::folder_listing(children, self.server_id()))
    }
}
