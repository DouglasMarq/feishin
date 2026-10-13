use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use crate::error::Result;

struct Remembered<Key, Value> {
    key: Key,
    value: Arc<Value>,
    fetched_at: Instant,
}

pub struct LatestCache<Key, Value> {
    time_to_live: Duration,
    latest: Mutex<Option<Remembered<Key, Value>>>,
}

impl<Key: PartialEq + Clone, Value> LatestCache<Key, Value> {
    pub fn new(time_to_live: Duration) -> Self {
        Self {
            time_to_live,
            latest: Mutex::new(None),
        }
    }

    pub async fn get_or_fetch<Fetch, Pending>(&self, key: &Key, fetch: Fetch) -> Result<Arc<Value>>
    where
        Fetch: FnOnce() -> Pending,
        Pending: Future<Output = Result<Value>>,
    {
        if let Some(value) = self.fresh(key) {
            return Ok(value);
        }
        let value = Arc::new(fetch().await?);
        *self.locked() = Some(Remembered {
            key: key.clone(),
            value: Arc::clone(&value),
            fetched_at: Instant::now(),
        });
        Ok(value)
    }

    pub fn invalidate(&self) {
        *self.locked() = None;
    }

    fn fresh(&self, key: &Key) -> Option<Arc<Value>> {
        self.locked()
            .as_ref()
            .filter(|remembered| {
                remembered.key == *key && remembered.fetched_at.elapsed() < self.time_to_live
            })
            .map(|remembered| Arc::clone(&remembered.value))
    }

    fn locked(&self) -> MutexGuard<'_, Option<Remembered<Key, Value>>> {
        self.latest.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    const MINUTE: Duration = Duration::from_secs(60);

    async fn cached(cache: &LatestCache<u8, u32>, key: u8, fetches: &AtomicUsize) -> u32 {
        cache
            .get_or_fetch(&key, || async {
                fetches.fetch_add(1, Ordering::SeqCst);
                Ok(u32::from(key) * 10)
            })
            .await
            .map_or(0, |value| *value)
    }

    #[tokio::test]
    async fn only_the_latest_key_is_kept() {
        let cache = LatestCache::new(MINUTE);
        let fetches = AtomicUsize::new(0);
        assert_eq!(cached(&cache, 1, &fetches).await, 10);
        assert_eq!(cached(&cache, 1, &fetches).await, 10);
        assert_eq!(fetches.load(Ordering::SeqCst), 1);
        assert_eq!(cached(&cache, 2, &fetches).await, 20);
        assert_eq!(cached(&cache, 1, &fetches).await, 10);
        assert_eq!(fetches.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn expired_or_invalidated_values_are_fetched_again() {
        let expiring = LatestCache::new(Duration::ZERO);
        let fetches = AtomicUsize::new(0);
        cached(&expiring, 1, &fetches).await;
        cached(&expiring, 1, &fetches).await;
        assert_eq!(fetches.load(Ordering::SeqCst), 2);

        let lasting = LatestCache::new(MINUTE);
        cached(&lasting, 1, &fetches).await;
        lasting.invalidate();
        cached(&lasting, 1, &fetches).await;
        assert_eq!(fetches.load(Ordering::SeqCst), 4);
    }
}
