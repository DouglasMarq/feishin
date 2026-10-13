use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use tokio::sync::OnceCell;

use crate::error::{ApiError, Result};

struct Fetched<Value> {
    value: Arc<Value>,
    fetched_at: Instant,
}

struct Slot<Key, Value> {
    key: Key,
    fetched: OnceCell<Fetched<Value>>,
}

type SharedSlot<Key, Value> = Arc<Slot<Key, Value>>;

pub struct LatestCache<Key, Value> {
    time_to_live: Duration,
    latest: Mutex<Option<SharedSlot<Key, Value>>>,
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
        let slot = self.slot_for(key);
        let fetched = slot
            .fetched
            .get_or_try_init(|| async {
                let value = fetch().await?;
                Ok::<_, ApiError>(Fetched {
                    value: Arc::new(value),
                    fetched_at: Instant::now(),
                })
            })
            .await?;
        Ok(Arc::clone(&fetched.value))
    }

    pub fn invalidate(&self) {
        *self.locked() = None;
    }

    fn slot_for(&self, key: &Key) -> SharedSlot<Key, Value> {
        let mut latest = self.locked();
        if let Some(slot) = latest.as_ref().filter(|slot| self.is_usable(slot, key)) {
            return Arc::clone(slot);
        }
        let slot = Arc::new(Slot {
            key: key.clone(),
            fetched: OnceCell::new(),
        });
        *latest = Some(Arc::clone(&slot));
        slot
    }

    fn is_usable(&self, slot: &Slot<Key, Value>, key: &Key) -> bool {
        slot.key == *key
            && slot
                .fetched
                .get()
                .is_none_or(|fetched| fetched.fetched_at.elapsed() < self.time_to_live)
    }

    fn locked(&self) -> MutexGuard<'_, Option<SharedSlot<Key, Value>>> {
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
                tokio::task::yield_now().await;
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

    #[tokio::test]
    async fn concurrent_requests_for_one_key_share_a_single_fetch() {
        let cache = LatestCache::new(MINUTE);
        let fetches = AtomicUsize::new(0);
        let (first, second) =
            tokio::join!(cached(&cache, 7, &fetches), cached(&cache, 7, &fetches));
        assert_eq!((first, second), (70, 70));
        assert_eq!(fetches.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_failed_fetch_is_retried_by_the_next_request() {
        let cache: LatestCache<u8, u32> = LatestCache::new(MINUTE);
        let failed = cache
            .get_or_fetch(&1, || async { Err(ApiError::NotFound) })
            .await;
        assert!(matches!(failed, Err(ApiError::NotFound)));
        let fetches = AtomicUsize::new(0);
        assert_eq!(cached(&cache, 1, &fetches).await, 10);
        assert_eq!(fetches.load(Ordering::SeqCst), 1);
    }
}
