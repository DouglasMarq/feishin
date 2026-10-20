use std::borrow::Borrow;
use std::collections::HashSet;
use std::hash::Hash;
use std::num::NonZeroUsize;

use lru::LruCache;

pub struct CoverLru<Key: Hash + Eq, Cover> {
    covers: LruCache<Key, Cover>,
    pending: HashSet<Key>,
    failed: HashSet<Key>,
}

impl<Key: Hash + Eq + Clone, Cover: Clone> CoverLru<Key, Cover> {
    pub fn new(capacity: usize) -> Self {
        Self {
            covers: LruCache::new(NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::MIN)),
            pending: HashSet::new(),
            failed: HashSet::new(),
        }
    }

    pub fn get<Query>(&mut self, key: &Query) -> Option<Cover>
    where
        Key: Borrow<Query>,
        Query: Hash + Eq + ?Sized,
    {
        self.covers.get(key).cloned()
    }

    pub fn peek<Query>(&self, key: &Query) -> Option<Cover>
    where
        Key: Borrow<Query>,
        Query: Hash + Eq + ?Sized,
    {
        self.covers.peek(key).cloned()
    }

    pub fn begin<Query>(&mut self, key: &Query) -> bool
    where
        Key: Borrow<Query>,
        Query: Hash + Eq + ToOwned<Owned = Key> + ?Sized,
    {
        !self.covers.contains(key)
            && !self.failed.contains(key)
            && !self.pending.contains(key)
            && self.pending.insert(key.to_owned())
    }

    pub fn insert(&mut self, key: Key, cover: Cover) -> Option<Key> {
        self.pending.remove(&key);
        self.covers
            .push(key.clone(), cover)
            .map(|(evicted, _)| evicted)
            .filter(|evicted| *evicted != key)
    }

    pub fn retry_later<Query>(&mut self, key: &Query)
    where
        Key: Borrow<Query>,
        Query: Hash + Eq + ?Sized,
    {
        self.pending.remove(key);
    }

    pub fn fail<Query>(&mut self, key: &Query)
    where
        Key: Borrow<Query>,
        Query: Hash + Eq + ToOwned<Owned = Key> + ?Sized,
    {
        self.pending.remove(key);
        self.failed.insert(key.to_owned());
    }

    pub fn clear(&mut self) {
        self.covers.clear();
        self.pending.clear();
        self.failed.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_are_bounded_and_evict_the_least_recently_used() {
        let mut covers = CoverLru::new(2);
        for index in 0..3 {
            assert!(covers.begin(&index));
        }
        covers.insert(0, 10);
        covers.insert(1, 11);
        covers.get(&0);
        assert_eq!(covers.insert(2, 12), Some(1));
        assert!(covers.get(&1).is_none() && covers.get(&2).is_some());
        assert!(covers.begin(&1));
        assert!(!covers.begin(&0));
    }

    #[test]
    fn lru_evicts_oldest_and_tracks_requests() {
        let mut covers = CoverLru::new(2);
        assert!(covers.begin("a"));
        assert!(!covers.begin("a"));
        covers.insert("a".to_owned(), 1);
        covers.insert("b".to_owned(), 2);
        assert_eq!(covers.get("a"), Some(1));
        covers.insert("c".to_owned(), 3);
        assert_eq!(covers.get("b"), None);
        assert_eq!(covers.get("a"), Some(1));
        assert!(covers.begin("d"));
        covers.fail("d");
        assert!(!covers.begin("d"));
        covers.clear();
        assert!(covers.begin("d"));
    }

    #[test]
    fn given_a_cover_replaced_in_place_nothing_is_evicted() {
        let mut covers = CoverLru::new(1);
        covers.insert(0, 1);
        assert_eq!(covers.insert(0, 2), None);
        assert_eq!(covers.get(&0), Some(2));
    }

    #[test]
    fn given_a_peek_the_recency_order_is_kept() {
        let mut covers = CoverLru::new(2);
        covers.insert(0, 1);
        covers.insert(1, 2);
        assert_eq!(covers.peek(&0), Some(1));
        assert_eq!(covers.insert(2, 3), Some(0));
    }

    #[test]
    fn given_a_cover_to_retry_later_it_can_be_requested_again() {
        let mut covers: CoverLru<usize, u8> = CoverLru::new(2);
        assert!(covers.begin(&0));
        covers.retry_later(&0);
        assert!(covers.begin(&0));
    }

    #[test]
    fn given_a_zero_capacity_one_cover_is_still_kept() {
        let mut covers = CoverLru::new(0);
        covers.insert(0, 1);
        assert_eq!(covers.get(&0), Some(1));
    }
}
