use std::collections::{HashMap, HashSet};

use feishin_core::model::{Paging, Song};

pub const PAGE_SIZE: usize = 100;
pub const MAX_CACHED_PAGES: usize = 8;

pub fn page_of(index: usize) -> usize {
    index / PAGE_SIZE
}

pub fn paging_for_page(page: usize) -> Paging {
    Paging {
        start_index: page * PAGE_SIZE,
        limit: PAGE_SIZE,
    }
}

#[derive(Default)]
pub struct PageStore {
    pages: HashMap<usize, Vec<Song>>,
    requested: HashSet<usize>,
    focus_page: usize,
}

impl PageStore {
    pub fn song(&self, index: usize) -> Option<&Song> {
        self.pages.get(&page_of(index))?.get(index % PAGE_SIZE)
    }

    pub fn focus_on(&mut self, index: usize) {
        self.focus_page = page_of(index);
    }

    pub fn request_once(&mut self, page: usize) -> bool {
        self.requested.insert(page)
    }

    pub fn request_failed(&mut self, page: usize) {
        self.requested.remove(&page);
    }

    pub fn insert(&mut self, page: usize, songs: Vec<Song>) -> bool {
        self.requested.insert(page);
        self.pages.insert(page, songs);
        self.evict_distant_pages();
        self.pages.contains_key(&page)
    }

    #[cfg(test)]
    pub fn cached_page_count(&self) -> usize {
        self.pages.len()
    }

    fn evict_distant_pages(&mut self) {
        while self.pages.len() > MAX_CACHED_PAGES {
            let focus_page = self.focus_page;
            let Some(farthest) = self
                .pages
                .keys()
                .copied()
                .max_by_key(|page| page.abs_diff(focus_page))
            else {
                return;
            };
            self.pages.remove(&farthest);
            self.requested.remove(&farthest);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::test_song;

    fn songs_for(page: usize) -> Vec<Song> {
        (0..PAGE_SIZE)
            .map(|position| test_song(&format!("s{}", page * PAGE_SIZE + position), 1000))
            .collect()
    }

    #[test]
    fn songs_are_found_by_absolute_index() {
        let mut store = PageStore::default();
        assert!(store.insert(2, songs_for(2)));
        assert_eq!(
            store.song(2 * PAGE_SIZE + 5).map(|song| song.id.as_str()),
            Some("s205")
        );
        assert!(store.song(5).is_none());
    }

    #[test]
    fn pages_far_from_the_focus_are_evicted() {
        let mut store = PageStore::default();
        store.focus_on(40 * PAGE_SIZE);
        for page in 0..MAX_CACHED_PAGES {
            store.insert(page, songs_for(page));
        }
        assert!(store.insert(40, songs_for(40)));
        assert_eq!(store.cached_page_count(), MAX_CACHED_PAGES);
        assert!(store.song(0).is_none());
        assert!(store.request_once(0));
        assert!(!store.request_once(40));
    }

    #[test]
    fn failed_requests_can_be_retried() {
        let mut store = PageStore::default();
        assert!(store.request_once(3));
        assert!(!store.request_once(3));
        store.request_failed(3);
        assert!(store.request_once(3));
    }
}
