use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use feishin_api::MusicServer;
use feishin_storage::Paths;
use feishin_storage::cover_cache::{CoverCache, CoverCacheConfig, DecodedImage};

use crate::demo::render_cover;

/// Cover art source (`feishin_storage::cover_cache::CoverCache` in real mode).
#[async_trait]
pub trait CoverProvider: Send + Sync {
    async fn cover(
        &self,
        server: &dyn MusicServer,
        cover_art_id: &str,
        size: u32,
    ) -> Result<Arc<DecodedImage>>;
}

pub struct CachedCovers(CoverCache);

impl CachedCovers {
    pub fn new(paths: &Paths) -> Result<Self> {
        let config = CoverCacheConfig {
            dir: paths.cache_dir.join("covers"),
            max_disk_bytes: 512 * 1024 * 1024,
            max_memory_items: 64,
        };
        Ok(Self(CoverCache::new(config, reqwest::Client::new())?))
    }
}

#[async_trait]
impl CoverProvider for CachedCovers {
    async fn cover(
        &self,
        server: &dyn MusicServer,
        cover_art_id: &str,
        size: u32,
    ) -> Result<Arc<DecodedImage>> {
        let key = format!("{}:{cover_art_id}", server.config().id);
        let url = server.cover_art_url(cover_art_id, Some(size));
        Ok(self.0.get(&key, &url, size).await?)
    }
}

pub struct DemoCovers;

#[async_trait]
impl CoverProvider for DemoCovers {
    async fn cover(
        &self,
        _server: &dyn MusicServer,
        cover_art_id: &str,
        size: u32,
    ) -> Result<Arc<DecodedImage>> {
        Ok(Arc::new(DecodedImage {
            width: size,
            height: size,
            rgba: render_cover(cover_art_id, size),
        }))
    }
}
