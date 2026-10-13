use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use lru::LruCache;
use tokio::sync::OnceCell;

use super::disk_index::DiskIndex;
use super::image_file::{cache_file_name, decode, mark_recently_used};
use super::locking::{lock, run_blocking};
use super::{CoverCacheConfig, DecodedImage};
use crate::{Result, StorageError, write_atomic};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct ImageKey {
    cover_key: String,
    size: u32,
}

impl ImageKey {
    pub(super) fn new(cover_key: &str, size: u32) -> Self {
        Self {
            cover_key: cover_key.to_string(),
            size,
        }
    }

    fn file_name(&self) -> String {
        cache_file_name(&self.cover_key, self.size)
    }
}

pub(super) type SharedLoad = std::result::Result<Arc<DecodedImage>, Arc<StorageError>>;

pub(super) struct CacheState {
    pub(super) config: CoverCacheConfig,
    pub(super) client: reqwest::Client,
    pub(super) memory: Mutex<Option<LruCache<ImageKey, Arc<DecodedImage>>>>,
    pub(super) disk: Mutex<DiskIndex>,
    pub(super) inflight: Mutex<HashMap<ImageKey, Arc<OnceCell<SharedLoad>>>>,
}

impl CacheState {
    pub(super) async fn load(self: Arc<Self>, image_key: ImageKey, url: String) -> SharedLoad {
        let image = self
            .load_uncached(&image_key, &url)
            .await
            .map_err(Arc::new)?;
        if let Some(memory) = lock(&self.memory).as_mut() {
            memory.put(image_key, Arc::clone(&image));
        }
        Ok(image)
    }

    async fn load_uncached(
        self: &Arc<Self>,
        image_key: &ImageKey,
        url: &str,
    ) -> Result<Arc<DecodedImage>> {
        let file_name = image_key.file_name();
        let size = image_key.size;
        let state = Arc::clone(self);
        let disk_file_name = file_name.clone();
        let from_disk =
            run_blocking(move || Ok(state.read_from_disk(&disk_file_name, size))).await?;
        if let Some(image) = from_disk {
            return Ok(Arc::new(image));
        }
        let image = self.download_and_store(url, file_name, size).await?;
        Ok(Arc::new(image))
    }

    async fn download_and_store(
        self: &Arc<Self>,
        url: &str,
        file_name: String,
        size: u32,
    ) -> Result<DecodedImage> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(network_error)?;
        let bytes = response.bytes().await.map_err(network_error)?;
        let state = Arc::clone(self);
        run_blocking(move || state.decode_and_store(&file_name, &bytes, size)).await
    }

    fn decode_and_store(&self, file_name: &str, bytes: &[u8], size: u32) -> Result<DecodedImage> {
        let image = decode(bytes, size)?;
        if let Err(error) = self.write_to_disk(file_name, bytes) {
            tracing::warn!("cover cache write failed: {error}");
        }
        Ok(image)
    }

    fn read_from_disk(&self, file_name: &str, size: u32) -> Option<DecodedImage> {
        if !lock(&self.disk).contains(file_name) {
            return None;
        }
        let path = self.config.dir.join(file_name);
        let decoded = std::fs::read(&path)
            .map_err(StorageError::from)
            .and_then(|bytes| decode(&bytes, size));
        let mut disk = lock(&self.disk);
        match decoded {
            Ok(image) => {
                disk.promote(file_name);
                mark_recently_used(&path);
                Some(image)
            }
            Err(error) => {
                tracing::debug!("dropping unreadable cover cache file {file_name}: {error}");
                disk.remove(&self.config.dir, file_name);
                None
            }
        }
    }

    fn write_to_disk(&self, file_name: &str, bytes: &[u8]) -> Result<()> {
        write_atomic(&self.config.dir.join(file_name), bytes)?;
        let mut disk = lock(&self.disk);
        disk.insert(file_name.to_string(), bytes.len() as u64);
        disk.evict(&self.config.dir, self.config.max_disk_bytes);
        Ok(())
    }
}

fn network_error(error: reqwest::Error) -> StorageError {
    StorageError::Network(error.to_string())
}
