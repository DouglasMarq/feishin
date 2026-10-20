use std::collections::HashSet;

use async_trait::async_trait;
use feishin_api::{MusicServer, Result};
use feishin_core::model::{Folder, FolderId, Page, Paging, Song};

const SKIP_CHUNK: usize = 500;

#[async_trait]
pub trait FolderSource: Send + Sync {
    async fn child_folders(&self, folder: &FolderId) -> Result<Vec<Folder>>;
    async fn song_page(&self, folder: &FolderId, paging: Paging) -> Result<Page<Song>>;
}

#[async_trait]
impl<Server: MusicServer + ?Sized> FolderSource for Server {
    async fn child_folders(&self, folder: &FolderId) -> Result<Vec<Folder>> {
        self.subfolders(folder).await
    }

    async fn song_page(&self, folder: &FolderId, paging: Paging) -> Result<Page<Song>> {
        self.folder_song_page(folder, paging).await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OpenFolder {
    folder: FolderId,
    next_song: usize,
}

#[derive(Debug)]
pub struct FolderWalk {
    pending: Vec<FolderId>,
    visited: HashSet<FolderId>,
    current: Option<OpenFolder>,
    gathered: Vec<Song>,
    walked: usize,
}

impl FolderWalk {
    pub fn new(root: FolderId) -> Self {
        Self {
            pending: vec![root],
            visited: HashSet::new(),
            current: None,
            gathered: Vec::new(),
            walked: 0,
        }
    }

    pub fn walked(&self) -> usize {
        self.walked
    }

    pub async fn skip<Source: FolderSource + ?Sized>(
        &mut self,
        source: &Source,
        count: usize,
    ) -> Result<()> {
        while self.walked < count {
            let wanted = (count - self.walked).min(SKIP_CHUNK);
            if self.next_songs(source, wanted).await?.is_empty() {
                return Ok(());
            }
        }
        Ok(())
    }

    pub async fn next_songs<Source: FolderSource + ?Sized>(
        &mut self,
        source: &Source,
        wanted: usize,
    ) -> Result<Vec<Song>> {
        while self.gathered.len() < wanted {
            let Some(open) = self.open_folder(source).await? else {
                break;
            };
            let paging = Paging {
                start_index: open.next_song,
                limit: wanted - self.gathered.len(),
            };
            let page = source.song_page(&open.folder, paging).await?;
            self.advance(open, page.items.len(), page.total_count, paging.limit);
            self.gathered.extend(page.items);
        }
        let handed_out = self.gathered.len().min(wanted);
        let songs: Vec<Song> = self.gathered.drain(..handed_out).collect();
        self.walked += songs.len();
        Ok(songs)
    }

    fn advance(&mut self, open: OpenFolder, received: usize, total: Option<usize>, asked: usize) {
        let next_song = open.next_song + received;
        let is_finished =
            received == 0 || total.map_or(received < asked, |total| next_song >= total);
        self.current = (!is_finished).then_some(OpenFolder { next_song, ..open });
    }

    async fn open_folder<Source: FolderSource + ?Sized>(
        &mut self,
        source: &Source,
    ) -> Result<Option<OpenFolder>> {
        if let Some(open) = &self.current {
            return Ok(Some(open.clone()));
        }
        while let Some(folder) = self.pending.last().cloned() {
            if self.visited.contains(&folder) {
                self.pending.pop();
                continue;
            }
            let subfolders = source.child_folders(&folder).await?;
            self.pending.pop();
            self.visited.insert(folder.clone());
            self.pending
                .extend(subfolders.iter().rev().map(Folder::folder_id));
            let open = OpenFolder {
                folder,
                next_song: 0,
            };
            self.current = Some(open.clone());
            return Ok(Some(open));
        }
        Ok(None)
    }
}
