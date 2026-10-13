use std::collections::{HashMap, HashSet};
use std::time::Instant;

use feishin_core::model::{Paging, Song};

use crate::ui::page_backoff::PageBackoff;

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
    backoff: PageBackoff,
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
        self.request_once_at(page, Instant::now())
    }

    pub fn request_once_at(&mut self, page: usize, now: Instant) -> bool {
        self.backoff.allows(page, now) && self.requested.insert(page)
    }

    pub fn request_failed(&mut self, page: usize) {
        self.request_failed_at(page, Instant::now());
    }

    pub fn request_failed_at(&mut self, page: usize, now: Instant) {
        if self.requested.remove(&page) {
            let delay = self.backoff.record_failure(page, now);
            tracing::debug!(
                page,
                ?delay,
                "a playlist page failed, waiting before asking again"
            );
        }
    }

    pub fn invalidate(&mut self) {
        self.pages.clear();
        self.requested.clear();
        self.backoff.clear();
    }

    pub fn insert(&mut self, page: usize, songs: Vec<Song>) -> bool {
        self.requested.insert(page);
        self.backoff.succeeded(page);
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
    fn failed_requests_are_retried_after_a_growing_wait() {
        let mut store = PageStore::default();
        let now = Instant::now();
        assert!(store.request_once_at(3, now));
        assert!(!store.request_once_at(3, now));
        store.request_failed_at(3, now);
        assert!(!store.request_once_at(3, now));
        let first_retry = now + std::time::Duration::from_secs(2);
        assert!(store.request_once_at(3, first_retry));
        store.request_failed_at(3, first_retry);
        assert!(!store.request_once_at(3, first_retry + std::time::Duration::from_secs(3)));
        assert!(store.request_once_at(3, first_retry + std::time::Duration::from_secs(4)));
    }

    #[test]
    fn invalidating_forgets_every_page() {
        let mut store = PageStore::default();
        assert!(store.insert(2, songs_for(2)));
        store.invalidate();
        assert!(store.song(2 * PAGE_SIZE).is_none());
        assert!(store.request_once(2));
    }
}
