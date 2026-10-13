use std::collections::{BTreeSet, HashMap};

use feishin_api::MusicServer;
use feishin_core::model::Song;

use super::pages::{PAGE_SIZE, page_of, paging_for_page};

pub struct TargetSnapshot {
    pub playlist_id: String,
    pub rows: Vec<(usize, Option<Song>)>,
}

impl TargetSnapshot {
    pub fn missing_pages(&self) -> BTreeSet<usize> {
        self.rows
            .iter()
            .filter(|(_, song)| song.is_none())
            .map(|(index, _)| page_of(*index))
            .collect()
    }

    pub fn is_loaded(&self) -> bool {
        self.rows.iter().all(|(_, song)| song.is_some())
    }

    pub fn into_loaded(self) -> Vec<Song> {
        self.rows.into_iter().filter_map(|(_, song)| song).collect()
    }

    pub async fn resolve(self, server: &dyn MusicServer) -> feishin_api::Result<Vec<Song>> {
        let pages = self.fetch_missing_pages(server).await?;
        Ok(self
            .rows
            .into_iter()
            .filter_map(|(index, song)| song.or_else(|| song_from_pages(&pages, index)))
            .collect())
    }

    async fn fetch_missing_pages(
        &self,
        server: &dyn MusicServer,
    ) -> feishin_api::Result<HashMap<usize, Vec<Song>>> {
        let mut pages = HashMap::new();
        for page in self.missing_pages() {
            let songs = server
                .playlist_song_page(&self.playlist_id, paging_for_page(page))
                .await?;
            pages.insert(page, songs.items);
        }
        Ok(pages)
    }
}

fn song_from_pages(pages: &HashMap<usize, Vec<Song>>, index: usize) -> Option<Song> {
    pages.get(&page_of(index))?.get(index % PAGE_SIZE).cloned()
}
