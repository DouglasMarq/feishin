use std::sync::Arc;

use async_trait::async_trait;
use feishin_api::MusicServer;
use feishin_core::model::{FolderId, Page, Paging, Song};
use tokio::sync::Mutex;

use super::folder_walk::{FolderSource, FolderWalk};
use super::source::PageFetcher;

struct WalkProgress {
    walk: FolderWalk,
    latest: Option<(Paging, Page<Song>)>,
}

pub struct FolderPages<Source: ?Sized = dyn MusicServer> {
    source: Arc<Source>,
    root: FolderId,
    progress: Mutex<WalkProgress>,
}

impl<Source: ?Sized> FolderPages<Source> {
    pub fn new(source: Arc<Source>, root: FolderId) -> Self {
        let progress = Mutex::new(WalkProgress {
            walk: FolderWalk::new(root.clone()),
            latest: None,
        });
        Self {
            source,
            root,
            progress,
        }
    }
}

pub async fn walk_page<Source: FolderSource + ?Sized>(
    walk: &mut FolderWalk,
    source: &Source,
    paging: Paging,
) -> feishin_api::Result<Page<Song>> {
    walk.skip(source, paging.start_index).await?;
    let items = walk.next_songs(source, paging.limit).await?;
    Ok(Page {
        items,
        start_index: paging.start_index,
        total_count: None,
    })
}

#[async_trait]
impl<Source: FolderSource + ?Sized + 'static> PageFetcher for FolderPages<Source> {
    async fn fetch(&self, paging: Paging) -> feishin_api::Result<Page<Song>> {
        let mut progress = self.progress.lock().await;
        if let Some((_, page)) = progress.latest.as_ref().filter(|(at, _)| *at == paging) {
            return Ok(page.clone());
        }
        if progress.walk.walked() != paging.start_index {
            tracing::debug!(
                "folder walk at {} restarts for row {}",
                progress.walk.walked(),
                paging.start_index
            );
            progress.walk = FolderWalk::new(self.root.clone());
        }
        let page = walk_page(&mut progress.walk, self.source.as_ref(), paging).await?;
        progress.latest = Some((paging, page.clone()));
        Ok(page)
    }
}
