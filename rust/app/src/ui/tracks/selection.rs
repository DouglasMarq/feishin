use std::collections::{BTreeSet, HashMap};

use feishin_api::MusicServer;
use feishin_core::model::{Song, SongListQuery};

use super::model::{PAGE_SIZE, paging_for_page};

#[derive(Debug, Default, Clone)]
pub struct Selection {
    rows: BTreeSet<usize>,
    anchor: Option<usize>,
}

impl Selection {
    pub fn contains(&self, index: usize) -> bool {
        self.rows.contains(&index)
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn rows(&self) -> impl Iterator<Item = usize> + '_ {
        self.rows.iter().copied()
    }

    pub fn click(&mut self, index: usize, toggle: bool, extend: bool) -> Vec<usize> {
        let before = self.rows.clone();
        match self.anchor.filter(|_| extend) {
            Some(anchor) => self.extend_from_anchor(anchor, index, toggle),
            None if toggle => self.toggle_row(index),
            None => self.select_only(index),
        }
        before.symmetric_difference(&self.rows).copied().collect()
    }

    pub fn focus(&mut self, index: usize) -> Vec<usize> {
        if self.contains(index) {
            return Vec::new();
        }
        self.click(index, false, false)
    }

    fn extend_from_anchor(&mut self, anchor: usize, index: usize, keep_existing: bool) {
        if !keep_existing {
            self.rows.clear();
        }
        self.rows.extend(anchor.min(index)..=anchor.max(index));
    }

    fn toggle_row(&mut self, index: usize) {
        if !self.rows.remove(&index) {
            self.rows.insert(index);
        }
        self.anchor = Some(index);
    }

    fn select_only(&mut self, index: usize) {
        self.rows = BTreeSet::from([index]);
        self.anchor = Some(index);
    }
}

pub struct SelectionSnapshot {
    pub(super) query: SongListQuery,
    pub(super) rows: Vec<(usize, Option<Song>)>,
}

impl SelectionSnapshot {
    pub fn missing_pages(&self) -> BTreeSet<usize> {
        self.rows
            .iter()
            .filter(|(_, song)| song.is_none())
            .map(|(index, _)| index / PAGE_SIZE)
            .collect()
    }

    pub fn is_loaded(&self) -> bool {
        self.rows.iter().all(|(_, song)| song.is_some())
    }

    pub fn first_only(mut self) -> Self {
        self.rows.truncate(1);
        self
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
            let songs = server.song_list(&self.query, paging_for_page(page)).await?;
            pages.insert(page, songs.items);
        }
        Ok(pages)
    }
}

fn song_from_pages(pages: &HashMap<usize, Vec<Song>>, index: usize) -> Option<Song> {
    pages
        .get(&(index / PAGE_SIZE))?
        .get(index % PAGE_SIZE)
        .cloned()
}
