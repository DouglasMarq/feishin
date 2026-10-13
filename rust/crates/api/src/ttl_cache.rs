use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use tokio::sync::OnceCell;

use crate::error::Result;

struct Fetched<Value> {
    value: Arc<Value>,
    fetched_at: Instant,
}

type Slot<Value> = Arc<OnceCell<Fetched<Value>>>;

pub struct TtlCache<Key, Value> {
    time_to_live: Duration,
    slots: Mutex<HashMap<Key, Slot<Value>>>,
}

fn is_expired<Value>(slot: &Slot<Value>, time_to_live: Duration) -> bool {
    slot.get()
        .is_some_and(|fetched| fetched.fetched_at.elapsed() >= time_to_live)
}

async fn fetch_now<Value, Pending>(pending: Pending) -> Result<Fetched<Value>>
where
    Pending: Future<Output = Result<Value>>,
{
    let value = pending.await?;
    Ok(Fetched {
        value: Arc::new(value),
        fetched_at: Instant::now(),
    })
}

impl<Key: Eq + Hash + Clone, Value> TtlCache<Key, Value> {
    pub fn new(time_to_live: Duration) -> Self {
        Self {
            time_to_live,
            slots: Mutex::new(HashMap::new()),
        }
    }

    pub async fn get_or_fetch<Fetch, Pending>(&self, key: &Key, fetch: Fetch) -> Result<Arc<Value>>
    where
        Fetch: FnOnce() -> Pending,
        Pending: Future<Output = Result<Value>>,
    {
        let slot = self.live_slot(key);
        let fetched = slot.get_or_try_init(|| fetch_now(fetch())).await?;
        Ok(Arc::clone(&fetched.value))
    }

    pub fn invalidate_all(&self) {
        self.locked_slots().clear();
    }

    fn live_slot(&self, key: &Key) -> Slot<Value> {
        let time_to_live = self.time_to_live;
        let mut slots = self.locked_slots();
        slots.retain(|_, slot| !is_expired(slot, time_to_live));
        Arc::clone(slots.entry(key.clone()).or_default())
    }

    fn locked_slots(&self) -> MutexGuard<'_, HashMap<Key, Slot<Value>>> {
        self.slots.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::error::ApiError;

    const MINUTE: Duration = Duration::from_secs(60);

    async fn counted_fetch(fetches: &AtomicUsize, value: u32) -> Result<u32> {
        fetches.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(5)).await;
        Ok(value)
    }

    async fn cached_value(cache: &TtlCache<u8, u32>, key: u8, fetches: &AtomicUsize) -> u32 {
        cache
            .get_or_fetch(&key, || counted_fetch(fetches, u32::from(key)))
            .await
            .map_or(0, |value| *value)
    }

    #[tokio::test]
    async fn concurrent_requests_for_one_key_fetch_once() {
        let cache = TtlCache::new(MINUTE);
        let fetches = AtomicUsize::new(0);
        let (first, second, third) = tokio::join!(
            cached_value(&cache, 7, &fetches),
            cached_value(&cache, 7, &fetches),
            cached_value(&cache, 7, &fetches),
        );
        assert_eq!((first, second, third), (7, 7, 7));
        assert_eq!(cached_value(&cache, 7, &fetches).await, 7);
        assert_eq!(fetches.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn keys_are_cached_separately() {
        let cache = TtlCache::new(MINUTE);
        let fetches = AtomicUsize::new(0);
        assert_eq!(cached_value(&cache, 1, &fetches).await, 1);
        assert_eq!(cached_value(&cache, 2, &fetches).await, 2);
        assert_eq!(cached_value(&cache, 1, &fetches).await, 1);
        assert_eq!(fetches.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn expired_and_invalidated_entries_are_fetched_again() {
        let expiring = TtlCache::new(Duration::ZERO);
        let fetches = AtomicUsize::new(0);
        cached_value(&expiring, 1, &fetches).await;
        cached_value(&expiring, 1, &fetches).await;
        assert_eq!(fetches.load(Ordering::SeqCst), 2);

        let lasting = TtlCache::new(MINUTE);
        cached_value(&lasting, 1, &fetches).await;
        lasting.invalidate_all();
        cached_value(&lasting, 1, &fetches).await;
        assert_eq!(fetches.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn failures_are_not_cached() {
        let cache: TtlCache<u8, u32> = TtlCache::new(MINUTE);
        let failed = cache
            .get_or_fetch(&1, || async { Err(ApiError::NotFound) })
            .await;
        assert!(matches!(failed, Err(ApiError::NotFound)));
        let fetches = AtomicUsize::new(0);
        assert_eq!(cached_value(&cache, 1, &fetches).await, 1);
        assert_eq!(fetches.load(Ordering::SeqCst), 1);
    }
}
