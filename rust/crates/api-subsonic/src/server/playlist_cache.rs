use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use feishin_api::Result;
use feishin_core::model::{Page, Paging, Playlist, Song};
use tokio::sync::Mutex;

const DEFAULT_TIME_TO_LIVE: Duration = Duration::from_secs(30);
const MAX_CACHED_PLAYLISTS: usize = 4;

pub struct CachedPlaylist {
    pub playlist: Playlist,
    songs: Box<[Song]>,
}

impl CachedPlaylist {
    pub fn new(playlist: Playlist, songs: Vec<Song>) -> Self {
        Self {
            playlist,
            songs: songs.into_boxed_slice(),
        }
    }

    pub fn songs(&self) -> &[Song] {
        &self.songs
    }

    pub fn song_page(&self, paging: Paging) -> Page<Song> {
        let total = self.songs.len();
        let start = paging.start_index.min(total);
        let end = match paging.limit {
            0 => total,
            limit => start.saturating_add(limit).min(total),
        };
        Page {
            items: self.songs.get(start..end).unwrap_or_default().to_vec(),
            start_index: paging.start_index,
            total_count: Some(total),
        }
    }
}

struct CacheEntry {
    stored_at: Instant,
    contents: Arc<CachedPlaylist>,
}

pub struct PlaylistCache {
    time_to_live: Duration,
    entries: Mutex<HashMap<String, CacheEntry>>,
}

impl Default for PlaylistCache {
    fn default() -> Self {
        Self::new(DEFAULT_TIME_TO_LIVE)
    }
}

impl PlaylistCache {
    pub fn new(time_to_live: Duration) -> Self {
        Self {
            time_to_live,
            entries: Mutex::default(),
        }
    }

    pub async fn get_or_load(
        &self,
        id: &str,
        load: impl Future<Output = Result<CachedPlaylist>>,
    ) -> Result<Arc<CachedPlaylist>> {
        let mut entries = self.entries.lock().await;
        if let Some(entry) = entries.get(id).filter(|entry| self.is_fresh(entry)) {
            return Ok(Arc::clone(&entry.contents));
        }
        let contents = Arc::new(load.await?);
        self.make_room(&mut entries);
        let entry = CacheEntry {
            stored_at: Instant::now(),
            contents: Arc::clone(&contents),
        };
        entries.insert(id.to_owned(), entry);
        Ok(contents)
    }

    pub async fn invalidate(&self, id: &str) {
        self.entries.lock().await.remove(id);
    }

    fn is_fresh(&self, entry: &CacheEntry) -> bool {
        entry.stored_at.elapsed() < self.time_to_live
    }

    fn make_room(&self, entries: &mut HashMap<String, CacheEntry>) {
        entries.retain(|_, entry| self.is_fresh(entry));
        while entries.len() >= MAX_CACHED_PLAYLISTS {
            let Some(oldest) = oldest_id(entries) else {
                return;
            };
            entries.remove(&oldest);
        }
    }
}

fn oldest_id(entries: &HashMap<String, CacheEntry>) -> Option<String> {
    entries
        .iter()
        .min_by_key(|(_, entry)| entry.stored_at)
        .map(|(id, _)| id.clone())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::normalize;
    use crate::types::{SubsonicPlaylist, SubsonicSong};

    fn cached(count: usize) -> CachedPlaylist {
        let songs = (0..count)
            .map(|index| {
                let entry = SubsonicSong {
                    id: format!("s{index}"),
                    ..SubsonicSong::default()
                };
                normalize::song(entry, "srv")
            })
            .collect();
        let playlist = normalize::playlist(SubsonicPlaylist::default(), "srv");
        CachedPlaylist::new(playlist, songs)
    }

    async fn load_counted(
        cache: &PlaylistCache,
        id: &str,
        loads: &AtomicUsize,
    ) -> Arc<CachedPlaylist> {
        cache
            .get_or_load(id, async {
                loads.fetch_add(1, Ordering::SeqCst);
                Ok(cached(3))
            })
            .await
            .unwrap()
    }

    #[test]
    fn pages_are_sliced_locally() {
        let playlist = cached(5);
        let page = playlist.song_page(Paging {
            start_index: 3,
            limit: 10,
        });
        assert_eq!(page.total_count, Some(5));
        assert_eq!(page.start_index, 3);
        let ids: Vec<&str> = page.items.iter().map(|song| song.id.as_str()).collect();
        assert_eq!(ids, ["s3", "s4"]);
        let past_end = playlist.song_page(Paging {
            start_index: 9,
            limit: 2,
        });
        assert!(past_end.items.is_empty());
    }

    #[tokio::test]
    async fn fresh_entries_are_reused_until_invalidated() {
        let cache = PlaylistCache::default();
        let loads = AtomicUsize::new(0);
        load_counted(&cache, "p1", &loads).await;
        load_counted(&cache, "p1", &loads).await;
        assert_eq!(loads.load(Ordering::SeqCst), 1);
        cache.invalidate("p1").await;
        load_counted(&cache, "p1", &loads).await;
        assert_eq!(loads.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn expired_entries_are_reloaded() {
        let cache = PlaylistCache::new(Duration::ZERO);
        let loads = AtomicUsize::new(0);
        load_counted(&cache, "p1", &loads).await;
        load_counted(&cache, "p1", &loads).await;
        assert_eq!(loads.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn the_cache_keeps_a_bounded_number_of_playlists() {
        let cache = PlaylistCache::default();
        let loads = AtomicUsize::new(0);
        for index in 0..10 {
            load_counted(&cache, &format!("p{index}"), &loads).await;
        }
        assert_eq!(cache.entries.lock().await.len(), MAX_CACHED_PLAYLISTS);
        load_counted(&cache, "p9", &loads).await;
        assert_eq!(loads.load(Ordering::SeqCst), 10);
    }
}
