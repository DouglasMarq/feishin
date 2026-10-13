use std::collections::{HashSet, VecDeque};

pub struct CoverLru<Cover> {
    capacity: usize,
    least_recent_first: VecDeque<(String, Cover)>,
    pending: HashSet<String>,
    failed: HashSet<String>,
}

impl<Cover: Clone> CoverLru<Cover> {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            least_recent_first: VecDeque::new(),
            pending: HashSet::new(),
            failed: HashSet::new(),
        }
    }

    pub fn get(&mut self, cover_art_id: &str) -> Option<Cover> {
        let position = self
            .least_recent_first
            .iter()
            .position(|(cached_id, _)| cached_id == cover_art_id)?;
        let (cached_id, cover) = self.least_recent_first.remove(position)?;
        self.least_recent_first
            .push_back((cached_id, cover.clone()));
        Some(cover)
    }

    fn is_known(&self, cover_art_id: &str) -> bool {
        self.failed.contains(cover_art_id)
            || self.pending.contains(cover_art_id)
            || self
                .least_recent_first
                .iter()
                .any(|(cached_id, _)| cached_id == cover_art_id)
    }

    pub fn begin(&mut self, cover_art_id: &str) -> bool {
        if self.is_known(cover_art_id) {
            return false;
        }
        self.pending.insert(cover_art_id.to_owned());
        true
    }

    pub fn finish(&mut self, cover_art_id: &str, cover: Option<Cover>) {
        self.pending.remove(cover_art_id);
        let Some(cover) = cover else {
            self.failed.insert(cover_art_id.to_owned());
            return;
        };
        self.least_recent_first
            .retain(|(cached_id, _)| cached_id != cover_art_id);
        self.least_recent_first
            .push_back((cover_art_id.to_owned(), cover));
        while self.least_recent_first.len() > self.capacity {
            self.least_recent_first.pop_front();
        }
    }

    pub fn clear(&mut self) {
        self.least_recent_first.clear();
        self.pending.clear();
        self.failed.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lru_evicts_oldest_and_tracks_requests() {
        let mut covers = CoverLru::new(2);
        assert!(covers.begin("a"));
        assert!(!covers.begin("a"));
        covers.finish("a", Some(1));
        covers.finish("b", Some(2));
        assert_eq!(covers.get("a"), Some(1));
        covers.finish("c", Some(3));
        assert_eq!(covers.get("b"), None);
        assert_eq!(covers.get("a"), Some(1));
        assert!(covers.begin("d"));
        covers.finish("d", None);
        assert!(!covers.begin("d"));
        covers.clear();
        assert!(covers.begin("d"));
    }
}
