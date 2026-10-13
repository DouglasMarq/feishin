use std::sync::Arc;

use feishin_api::Result;
use feishin_core::model::{Folder, FolderId, Page, Paging, Song};

use super::SubsonicServer;
use crate::client::Params;
use crate::normalize::{self, FolderListing};
use crate::types::{GetIndexes, GetMusicDirectory};

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

impl SubsonicServer {
    pub(super) async fn fetch_subfolders(&self, folder: &FolderId) -> Result<Vec<Folder>> {
        Ok(self.folder_listing(folder).await?.folders.clone())
    }

    pub(super) async fn fetch_folder_song_page(
        &self,
        folder: &FolderId,
        paging: Paging,
    ) -> Result<Page<Song>> {
        let listing = self.folder_listing(folder).await?;
        Ok(song_page(&listing.songs, paging))
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
        let response: GetIndexes = self.fetch("getIndexes.view", Params::new()).await?;
        let indexes = response.indexes.unwrap_or_default();
        let mut listing =
            normalize::folder_listing(indexes.child.unwrap_or_default(), self.server_id());
        listing.folders = indexes
            .index
            .unwrap_or_default()
            .into_iter()
            .flat_map(|index| index.artist.unwrap_or_default())
            .map(normalize::indexed_folder)
            .chain(listing.folders)
            .collect();
        Ok(listing)
    }

    async fn fetch_music_directory(&self, id: &str) -> Result<FolderListing> {
        let response: GetMusicDirectory = self
            .fetch("getMusicDirectory.view", Params::new().add("id", id))
            .await?;
        let children = response
            .directory
            .and_then(|directory| directory.child)
            .unwrap_or_default();
        Ok(normalize::folder_listing(children, self.server_id()))
    }
}
