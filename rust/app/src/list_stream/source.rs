use std::sync::Arc;

use async_trait::async_trait;
use feishin_api::MusicServer;
use feishin_core::model::{FolderId, Page, Paging, Song, SongListQuery};

use super::folder_pages::FolderPages;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistId(pub String);

#[derive(Debug, Clone, PartialEq)]
pub enum SongSource {
    Library(SongListQuery),
    Playlist(PlaylistId),
    Folder(FolderId),
    FolderTree(FolderId),
}

#[async_trait]
pub trait PageFetcher: Send + Sync {
    async fn fetch(&self, paging: Paging) -> feishin_api::Result<Page<Song>>;
}

pub struct ServerPages {
    pub server: Arc<dyn MusicServer>,
    pub source: SongSource,
}

#[async_trait]
impl PageFetcher for ServerPages {
    async fn fetch(&self, paging: Paging) -> feishin_api::Result<Page<Song>> {
        crate::library::song_page(self.server.as_ref(), &self.source, paging).await
    }
}

pub fn page_fetcher(server: Arc<dyn MusicServer>, source: SongSource) -> Box<dyn PageFetcher> {
    match source {
        SongSource::FolderTree(root) => Box::new(FolderPages::new(server, root)),
        listed => Box::new(ServerPages {
            server,
            source: listed,
        }),
    }
}
