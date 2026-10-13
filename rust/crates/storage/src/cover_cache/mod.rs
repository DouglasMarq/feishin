mod cache_state;
mod disk_index;
mod image_file;
mod locking;

use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use lru::LruCache;
use tokio::sync::OnceCell;

use crate::Result;
use cache_state::{CacheState, ImageKey, SharedLoad};
use disk_index::DiskIndex;
use locking::lock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct CoverCacheConfig {
    pub dir: PathBuf,
    pub max_disk_bytes: u64,
    pub max_memory_items: usize,
}

pub struct CoverCache {
    inner: Arc<CacheState>,
}

impl CoverCache {
    pub fn new(config: CoverCacheConfig, client: reqwest::Client) -> Result<Self> {
        std::fs::create_dir_all(&config.dir)?;
        let mut disk = DiskIndex::scan(&config.dir)?;
        disk.evict(&config.dir, config.max_disk_bytes);
        let memory = NonZeroUsize::new(config.max_memory_items).map(LruCache::new);
        Ok(Self {
            inner: Arc::new(CacheState {
                config,
                client,
                memory: Mutex::new(memory),
                disk: Mutex::new(disk),
                inflight: Mutex::new(HashMap::new()),
            }),
        })
    }

    pub async fn get(&self, key: &str, url: &str, size: u32) -> Result<Arc<DecodedImage>> {
        if let Some(image) = self.get_cached(key, size) {
            return Ok(image);
        }
        let image_key = ImageKey::new(key, size);
        let load_cell = self.inflight_load(&image_key);
        let shared_result = load_cell
            .get_or_init(|| Arc::clone(&self.inner).load(image_key.clone(), url.to_string()))
            .await
            .clone();
        self.forget_inflight_load(&image_key, &load_cell);
        shared_result.map_err(|error| error.duplicate())
    }

    pub fn get_cached(&self, key: &str, size: u32) -> Option<Arc<DecodedImage>> {
        lock(&self.inner.memory)
            .as_mut()?
            .get(&ImageKey::new(key, size))
            .cloned()
    }

    pub fn disk_usage(&self) -> u64 {
        lock(&self.inner.disk).total_bytes()
    }

    fn inflight_load(&self, image_key: &ImageKey) -> Arc<OnceCell<SharedLoad>> {
        lock(&self.inner.inflight)
            .entry(image_key.clone())
            .or_default()
            .clone()
    }

    fn forget_inflight_load(&self, image_key: &ImageKey, load_cell: &Arc<OnceCell<SharedLoad>>) {
        let mut inflight = lock(&self.inner.inflight);
        let is_same_load = inflight
            .get(image_key)
            .is_some_and(|current| Arc::ptr_eq(current, load_cell));
        if is_same_load {
            inflight.remove(image_key);
        }
    }
}

#[cfg(test)]
mod tests;
