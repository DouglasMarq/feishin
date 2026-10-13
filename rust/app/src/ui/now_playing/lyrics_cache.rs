use std::collections::VecDeque;
use std::rc::Rc;

use super::song_lyrics::SongLyrics;

pub(super) type CachedLyrics = Option<Rc<SongLyrics>>;

pub(super) struct LyricsCache {
    entries: VecDeque<(String, CachedLyrics)>,
    capacity: usize,
}

impl LyricsCache {
    pub(super) fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub(super) fn get(&mut self, song_id: &str) -> Option<CachedLyrics> {
        let index = self.entries.iter().position(|(id, _)| id == song_id)?;
        let (cached_song_id, lyrics) = self.entries.remove(index)?;
        self.entries.push_back((cached_song_id, lyrics.clone()));
        Some(lyrics)
    }

    pub(super) fn insert(&mut self, song_id: String, lyrics: CachedLyrics) {
        self.entries.retain(|(id, _)| *id != song_id);
        if self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back((song_id, lyrics));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_is_bounded_and_keeps_recent_entries() {
        let mut cache = LyricsCache::new(2);
        cache.insert("a".into(), None);
        cache.insert("b".into(), None);
        assert_eq!(cache.get("a"), Some(None));
        cache.insert("c".into(), None);
        assert!(cache.get("b").is_none());
        assert!(cache.get("a").is_some());
        assert!(cache.get("c").is_some());
    }
}
