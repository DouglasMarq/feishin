use std::sync::Arc;

use async_trait::async_trait;
use feishin_api::MusicServer;
use feishin_core::model::{FolderId, Page, Paging, Song};
use tokio::sync::Mutex;

use super::folder_walk::FolderWalk;
use super::source::PageFetcher;

pub struct FolderPages {
    server: Arc<dyn MusicServer>,
    root: FolderId,
    walk: Mutex<FolderWalk>,
}

impl FolderPages {
    pub fn new(server: Arc<dyn MusicServer>, root: FolderId) -> Self {
        let walk = Mutex::new(FolderWalk::new(root.clone()));
        Self { server, root, walk }
    }
}

pub async fn walk_page(
    walk: &mut FolderWalk,
    server: &dyn MusicServer,
    paging: Paging,
) -> feishin_api::Result<Page<Song>> {
    walk.skip(server, paging.start_index).await?;
    let items = walk.next_songs(server, paging.limit).await?;
    Ok(Page {
        items,
        start_index: paging.start_index,
        total_count: None,
    })
}

#[async_trait]
impl PageFetcher for FolderPages {
    async fn fetch(&self, paging: Paging) -> feishin_api::Result<Page<Song>> {
        let mut walk = self.walk.lock().await;
        if walk.walked() != paging.start_index {
            tracing::debug!(
                "folder walk at {} restarts for row {}",
                walk.walked(),
                paging.start_index
            );
            *walk = FolderWalk::new(self.root.clone());
        }
        let before = walk.clone();
        let fetched = walk_page(&mut walk, self.server.as_ref(), paging).await;
        if fetched.is_err() {
            *walk = before;
        }
        fetched
    }
}
