use std::collections::{HashMap, HashSet, VecDeque};

use slint::Image;

pub struct CoverLru {
    capacity: usize,
    images: HashMap<usize, Image>,
    least_recent_first: VecDeque<usize>,
    pending: HashSet<usize>,
}

impl CoverLru {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            images: HashMap::new(),
            least_recent_first: VecDeque::new(),
            pending: HashSet::new(),
        }
    }

    pub fn get(&mut self, index: usize) -> Option<Image> {
        let image = self.images.get(&index)?.clone();
        if let Some(position) = self
            .least_recent_first
            .iter()
            .position(|&cached_index| cached_index == index)
        {
            self.least_recent_first.remove(position);
        }
        self.least_recent_first.push_back(index);
        Some(image)
    }

    pub fn begin(&mut self, index: usize) -> bool {
        !self.images.contains_key(&index) && self.pending.insert(index)
    }

    pub fn failed(&mut self, index: usize) {
        self.pending.remove(&index);
    }

    pub fn insert(&mut self, index: usize, image: Image) -> Vec<usize> {
        self.pending.remove(&index);
        if self.images.insert(index, image).is_none() {
            self.least_recent_first.push_back(index);
        }
        let mut evicted = Vec::new();
        while self.least_recent_first.len() > self.capacity {
            if let Some(oldest) = self.least_recent_first.pop_front() {
                self.images.remove(&oldest);
                evicted.push(oldest);
            }
        }
        evicted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_are_bounded_and_evict_the_least_recently_used() {
        let mut covers = CoverLru::new(2);
        for index in 0..3 {
            assert!(covers.begin(index));
        }
        covers.insert(0, Image::default());
        covers.insert(1, Image::default());
        covers.get(0);
        assert_eq!(covers.insert(2, Image::default()), [1]);
        assert!(covers.get(1).is_none() && covers.get(2).is_some());
        assert!(covers.begin(1));
        assert!(!covers.begin(0));
    }
}
