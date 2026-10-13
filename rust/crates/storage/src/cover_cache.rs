//! Cover art cache: files on disk plus a bounded in-memory LRU of decoded images.

use std::path::PathBuf;
use std::sync::Arc;

use crate::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    /// RGBA8, row-major.
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct CoverCacheConfig {
    pub dir: PathBuf,
    pub max_disk_bytes: u64,
    /// Decoded images kept in memory.
    pub max_memory_items: usize,
}

pub struct CoverCache {
    _private: (),
}

impl CoverCache {
    pub fn new(_config: CoverCacheConfig, _client: reqwest::Client) -> Result<Self> {
        todo!()
    }

    /// Returns the image for `key`, from memory, disk, or downloading `url`. Decoded and resized to fit `size` x `size`.
    pub async fn get(&self, _key: &str, _url: &str, _size: u32) -> Result<Arc<DecodedImage>> {
        todo!()
    }

    /// Memory-only lookup for synchronous UI code.
    pub fn get_cached(&self, _key: &str, _size: u32) -> Option<Arc<DecodedImage>> {
        todo!()
    }
}
