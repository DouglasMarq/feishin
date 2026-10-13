use std::collections::HashSet;

use feishin_api::{MusicServer, Result};
use feishin_core::model::{Folder, FolderId, Paging, Song};

const SKIP_CHUNK: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
struct OpenFolder {
    folder: FolderId,
    next_song: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderWalk {
    pending: Vec<FolderId>,
    visited: HashSet<FolderId>,
    current: Option<OpenFolder>,
    walked: usize,
}

impl FolderWalk {
    pub fn new(root: FolderId) -> Self {
        Self {
            pending: vec![root],
            visited: HashSet::new(),
            current: None,
            walked: 0,
        }
    }

    pub fn walked(&self) -> usize {
        self.walked
    }

    pub async fn skip(&mut self, server: &dyn MusicServer, count: usize) -> Result<()> {
        while self.walked < count {
            let wanted = (count - self.walked).min(SKIP_CHUNK);
            if self.next_songs(server, wanted).await?.is_empty() {
                return Ok(());
            }
        }
        Ok(())
    }

    pub async fn next_songs(
        &mut self,
        server: &dyn MusicServer,
        wanted: usize,
    ) -> Result<Vec<Song>> {
        let mut songs = Vec::new();
        while songs.len() < wanted {
            let Some(open) = self.open_folder(server).await? else {
                break;
            };
            let paging = Paging {
                start_index: open.next_song,
                limit: wanted - songs.len(),
            };
            let page = server.folder_song_page(&open.folder, paging).await?;
            self.advance(open, page.items.len(), page.total_count, paging.limit);
            songs.extend(page.items);
        }
        Ok(songs)
    }

    fn advance(&mut self, open: OpenFolder, received: usize, total: Option<usize>, asked: usize) {
        let next_song = open.next_song + received;
        let finished = received == 0 || total.map_or(received < asked, |total| next_song >= total);
        self.walked += received;
        self.current = (!finished).then_some(OpenFolder { next_song, ..open });
    }

    async fn open_folder(&mut self, server: &dyn MusicServer) -> Result<Option<OpenFolder>> {
        if let Some(open) = self.current.take() {
            return Ok(Some(open));
        }
        while let Some(folder) = self.pending.pop() {
            if !self.visited.insert(folder.clone()) {
                continue;
            }
            let subfolders = server.subfolders(&folder).await?;
            self.pending
                .extend(subfolders.iter().rev().map(Folder::folder_id));
            return Ok(Some(OpenFolder {
                folder,
                next_song: 0,
            }));
        }
        Ok(None)
    }
}
