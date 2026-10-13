use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};

use slint::Image;

const SHARED_COVER_CAPACITY: usize = 96;

pub struct CoverLru {
    images: HashMap<String, Image>,
    order: VecDeque<String>,
    capacity: usize,
}

impl CoverLru {
    pub fn new(capacity: usize) -> Self {
        Self {
            images: HashMap::new(),
            order: VecDeque::new(),
            capacity,
        }
    }

    pub fn get(&mut self, cover_id: &str) -> Option<Image> {
        let image = self.images.get(cover_id)?.clone();
        self.mark_recently_used(cover_id);
        Some(image)
    }

    pub fn insert(&mut self, cover_id: String, image: Image) {
        if self.images.insert(cover_id.clone(), image).is_none() {
            self.order.push_back(cover_id);
        }
        self.evict_over_capacity();
    }

    fn mark_recently_used(&mut self, cover_id: &str) {
        let used = self
            .order
            .iter()
            .position(|cached_id| cached_id == cover_id)
            .and_then(|position| self.order.remove(position));
        if let Some(used) = used {
            self.order.push_back(used);
        }
    }

    fn evict_over_capacity(&mut self) {
        while self.order.len() > self.capacity {
            if let Some(evicted) = self.order.pop_front() {
                self.images.remove(&evicted);
            }
        }
    }
}

thread_local! {
    static SHARED_COVERS: RefCell<CoverLru> = RefCell::new(CoverLru::new(SHARED_COVER_CAPACITY));
}

pub fn cached_cover(cover_id: &str) -> Option<Image> {
    SHARED_COVERS.with_borrow_mut(|covers| covers.get(cover_id))
}

pub fn remember_cover(cover_id: String, image: Image) {
    SHARED_COVERS.with_borrow_mut(|covers| covers.insert(cover_id, image));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_cache_evicts_least_recently_used() {
        let mut covers = CoverLru::new(2);
        covers.insert("a".into(), Image::default());
        covers.insert("b".into(), Image::default());
        assert!(covers.get("a").is_some());
        covers.insert("c".into(), Image::default());
        assert!(covers.get("b").is_none());
        assert!(covers.get("a").is_some() && covers.get("c").is_some());
    }
}
