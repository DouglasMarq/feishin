use std::collections::{HashSet, VecDeque};
use std::hash::Hash;

pub const MAX_SHOWN_COVERS: usize = 48;

pub struct CoverSlots<Key> {
    pending: HashSet<Key>,
    shown: VecDeque<Key>,
    capacity: usize,
}

impl<Key: Clone + Eq + Hash> Default for CoverSlots<Key> {
    fn default() -> Self {
        Self::with_capacity(MAX_SHOWN_COVERS)
    }
}

impl<Key: Clone + Eq + Hash> CoverSlots<Key> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            pending: HashSet::new(),
            shown: VecDeque::new(),
            capacity,
        }
    }

    pub fn claim(&mut self, key: Key) -> bool {
        !self.shown.contains(&key) && self.pending.insert(key)
    }

    pub fn finish(&mut self, key: &Key) -> bool {
        self.pending.remove(key)
    }

    pub fn show(&mut self, key: Key) -> Vec<Key> {
        self.shown.retain(|shown| *shown != key);
        self.shown.push_back(key);
        let excess = self.shown.len().saturating_sub(self.capacity);
        self.shown.drain(..excess).collect()
    }

    pub fn clear(&mut self) {
        self.pending.clear();
        self.shown.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_are_claimed_once_and_the_oldest_are_released() {
        let mut slots = CoverSlots::with_capacity(2);
        assert!(slots.claim("a"));
        assert!(!slots.claim("a"));
        assert!(slots.finish(&"a"));
        assert!(!slots.finish(&"a"));
        assert!(slots.show("a").is_empty());
        assert!(!slots.claim("a"), "already shown");
        assert!(slots.show("b").is_empty());
        assert_eq!(slots.show("c"), ["a"]);
        assert!(slots.claim("a"));
        slots.clear();
        assert!(slots.claim("b"));
    }
}
