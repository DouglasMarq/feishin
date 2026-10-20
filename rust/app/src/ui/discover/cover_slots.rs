use std::collections::{HashSet, VecDeque};
use std::hash::Hash;

pub const MAX_SHOWN_COVERS: usize = 48;

pub struct CoverSlots<Key> {
    pending: HashSet<Key>,
    shown: VecDeque<Key>,
    visible: HashSet<Key>,
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
            visible: HashSet::new(),
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
        self.evict_hidden_excess()
    }

    pub fn set_visible(&mut self, key: Key, is_visible: bool) -> Vec<Key> {
        match is_visible {
            true => {
                self.visible.insert(key);
            }
            false => {
                self.visible.remove(&key);
            }
        }
        self.evict_hidden_excess()
    }

    pub fn show_only(&mut self, visible: impl IntoIterator<Item = Key>) -> Vec<Key> {
        self.visible = visible.into_iter().collect();
        self.evict_hidden_excess()
    }

    fn evict_hidden_excess(&mut self) -> Vec<Key> {
        let mut excess = self.shown.len().saturating_sub(self.capacity);
        let mut evicted = Vec::new();
        let visible = &self.visible;
        self.shown.retain(|shown| {
            let should_evict = excess > 0 && !visible.contains(shown);
            if should_evict {
                excess -= 1;
                evicted.push(shown.clone());
            }
            !should_evict
        });
        evicted
    }

    pub fn forget_covers(&mut self) {
        self.pending.clear();
        self.shown.clear();
    }

    pub fn clear(&mut self) {
        self.forget_covers();
        self.visible.clear();
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

    #[test]
    fn covers_on_screen_are_kept_even_beyond_the_capacity() {
        let mut slots = CoverSlots::with_capacity(2);
        for key in ["a", "b", "c"] {
            assert!(slots.set_visible(key, true).is_empty());
            assert!(slots.show(key).is_empty(), "{key} is on screen");
        }
        assert!(slots.set_visible("d", false).is_empty());
        assert_eq!(slots.set_visible("b", false), ["b"]);
        assert!(!slots.claim("a"), "still shown");
        assert!(slots.show_only(["c"]).is_empty());
        assert_eq!(slots.show("e"), ["a"]);
        assert!(slots.claim("a"));
    }
}
