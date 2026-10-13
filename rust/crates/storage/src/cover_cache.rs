//! Cover art cache: files on disk plus a bounded in-memory LRU of decoded images.

use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::SystemTime;

use image::imageops::FilterType;
use lru::LruCache;
use sha2::{Digest, Sha256};
use tokio::sync::OnceCell;

use crate::{Result, StorageError, write_atomic};

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

type MemKey = (String, u32);
type Shared = std::result::Result<Arc<DecodedImage>, Arc<StorageError>>;

pub struct CoverCache {
    inner: Arc<Inner>,
}

struct Inner {
    config: CoverCacheConfig,
    client: reqwest::Client,
    memory: Mutex<Option<LruCache<MemKey, Arc<DecodedImage>>>>,
    disk: Mutex<DiskIndex>,
    inflight: Mutex<HashMap<MemKey, Arc<OnceCell<Shared>>>>,
}

struct DiskIndex {
    files: LruCache<String, u64>,
    total: u64,
}

impl CoverCache {
    pub fn new(config: CoverCacheConfig, client: reqwest::Client) -> Result<Self> {
        std::fs::create_dir_all(&config.dir)?;
        let mut disk = DiskIndex::scan(&config.dir)?;
        disk.evict(&config.dir, config.max_disk_bytes);
        let memory = NonZeroUsize::new(config.max_memory_items).map(LruCache::new);
        Ok(Self {
            inner: Arc::new(Inner {
                config,
                client,
                memory: Mutex::new(memory),
                disk: Mutex::new(disk),
                inflight: Mutex::new(HashMap::new()),
            }),
        })
    }

    /// Returns the image for `key`, from memory, disk, or downloading `url`. Decoded and resized to fit `size` x `size`.
    pub async fn get(&self, key: &str, url: &str, size: u32) -> Result<Arc<DecodedImage>> {
        if let Some(image) = self.get_cached(key, size) {
            return Ok(image);
        }
        let mem_key: MemKey = (key.to_string(), size);
        let cell = lock(&self.inner.inflight)
            .entry(mem_key.clone())
            .or_default()
            .clone();
        let result = cell
            .get_or_init(|| Arc::clone(&self.inner).load(mem_key.clone(), url.to_string()))
            .await
            .clone();
        {
            let mut inflight = lock(&self.inner.inflight);
            if inflight
                .get(&mem_key)
                .is_some_and(|current| Arc::ptr_eq(current, &cell))
            {
                inflight.remove(&mem_key);
            }
        }
        result.map_err(|e| clone_error(&e))
    }

    /// Memory-only lookup for synchronous UI code.
    pub fn get_cached(&self, key: &str, size: u32) -> Option<Arc<DecodedImage>> {
        lock(&self.inner.memory)
            .as_mut()?
            .get(&(key.to_string(), size))
            .cloned()
    }

    /// Bytes currently used on disk by cached files.
    pub fn disk_usage(&self) -> u64 {
        lock(&self.inner.disk).total
    }
}

impl Inner {
    async fn load(self: Arc<Self>, mem_key: MemKey, url: String) -> Shared {
        let image = self.load_uncached(&mem_key, &url).await.map_err(Arc::new)?;
        if let Some(memory) = lock(&self.memory).as_mut() {
            memory.put(mem_key, Arc::clone(&image));
        }
        Ok(image)
    }

    async fn load_uncached(
        self: &Arc<Self>,
        mem_key: &MemKey,
        url: &str,
    ) -> Result<Arc<DecodedImage>> {
        let file_name = file_name(&mem_key.0, mem_key.1);
        let size = mem_key.1;

        let this = Arc::clone(self);
        let name = file_name.clone();
        let from_disk = blocking(move || Ok(this.read_from_disk(&name, size))).await?;
        if let Some(image) = from_disk {
            return Ok(Arc::new(image));
        }

        let response = self
            .client
            .get(url)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|e| StorageError::Network(e.to_string()))?;
        let bytes = response
            .bytes()
            .await
            .map_err(|e| StorageError::Network(e.to_string()))?;

        let this = Arc::clone(self);
        let image = blocking(move || {
            let image = decode(&bytes, size)?;
            if let Err(e) = this.write_to_disk(&file_name, &bytes) {
                tracing::warn!("cover cache write failed: {e}");
            }
            Ok(image)
        })
        .await?;
        Ok(Arc::new(image))
    }

    fn read_from_disk(&self, name: &str, size: u32) -> Option<DecodedImage> {
        if !lock(&self.disk).files.contains(name) {
            return None;
        }
        let path = self.config.dir.join(name);
        let decoded = std::fs::read(&path)
            .map_err(StorageError::from)
            .and_then(|bytes| decode(&bytes, size));
        let mut disk = lock(&self.disk);
        match decoded {
            Ok(image) => {
                disk.files.promote(name);
                touch(&path);
                Some(image)
            }
            Err(e) => {
                tracing::debug!("dropping unreadable cover cache file {name}: {e}");
                disk.remove(&self.config.dir, name);
                None
            }
        }
    }

    fn write_to_disk(&self, name: &str, bytes: &[u8]) -> Result<()> {
        write_atomic(&self.config.dir.join(name), bytes)?;
        let mut disk = lock(&self.disk);
        disk.insert(name.to_string(), bytes.len() as u64);
        disk.evict(&self.config.dir, self.config.max_disk_bytes);
        Ok(())
    }
}

impl DiskIndex {
    fn scan(dir: &Path) -> Result<Self> {
        let mut found = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if !metadata.is_file() {
                continue;
            }
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            if name.starts_with(".tmp-") {
                let _ = std::fs::remove_file(entry.path());
                continue;
            }
            let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            found.push((modified, name, metadata.len()));
        }
        found.sort();
        let mut index = Self {
            files: LruCache::unbounded(),
            total: 0,
        };
        for (_, name, len) in found {
            index.insert(name, len);
        }
        Ok(index)
    }

    fn insert(&mut self, name: String, len: u64) {
        if let Some(previous) = self.files.put(name, len) {
            self.total = self.total.saturating_sub(previous);
        }
        self.total += len;
    }

    fn remove(&mut self, dir: &Path, name: &str) {
        if let Some(len) = self.files.pop(name) {
            self.total = self.total.saturating_sub(len);
        }
        let _ = std::fs::remove_file(dir.join(name));
    }

    fn evict(&mut self, dir: &Path, max_bytes: u64) {
        while self.total > max_bytes {
            let Some((name, len)) = self.files.pop_lru() else {
                break;
            };
            self.total = self.total.saturating_sub(len);
            if let Err(e) = std::fs::remove_file(dir.join(&name))
                && e.kind() != std::io::ErrorKind::NotFound
            {
                tracing::warn!("cover cache eviction failed for {name}: {e}");
            }
        }
    }
}

fn decode(bytes: &[u8], size: u32) -> Result<DecodedImage> {
    let mut image = image::load_from_memory(bytes)
        .map_err(|e| StorageError::Invalid(format!("cover image: {e}")))?;
    if size > 0 && (image.width() > size || image.height() > size) {
        image = image.resize(size, size, FilterType::CatmullRom);
    }
    let rgba = image.into_rgba8();
    Ok(DecodedImage {
        width: rgba.width(),
        height: rgba.height(),
        rgba: rgba.into_raw(),
    })
}

fn file_name(key: &str, size: u32) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hasher.update([0]);
    hasher.update(size.to_le_bytes());
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn touch(path: &Path) {
    if let Ok(file) = std::fs::File::options().append(true).open(path) {
        let _ = file.set_modified(SystemTime::now());
    }
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| StorageError::Invalid(format!("cover cache task failed: {e}")))?
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn clone_error(e: &StorageError) -> StorageError {
    match e {
        StorageError::Io(io) => StorageError::Io(std::io::Error::new(io.kind(), io.to_string())),
        StorageError::Invalid(m) => StorageError::Invalid(m.clone()),
        StorageError::Keyring(m) => StorageError::Keyring(m.clone()),
        StorageError::Network(m) => StorageError::Network(m.clone()),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::time::Duration;

    use image::{ImageFormat, Rgba, RgbaImage};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let image = RgbaImage::from_pixel(width, height, Rgba([200, 10, 30, 255]));
        let mut out = Cursor::new(Vec::new());
        image.write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn config(dir: &Path, max_disk_bytes: u64, max_memory_items: usize) -> CoverCacheConfig {
        CoverCacheConfig {
            dir: dir.to_path_buf(),
            max_disk_bytes,
            max_memory_items,
        }
    }

    fn cache(dir: &Path, max_disk_bytes: u64, max_memory_items: usize) -> CoverCache {
        CoverCache::new(
            config(dir, max_disk_bytes, max_memory_items),
            reqwest::Client::new(),
        )
        .unwrap()
    }

    async fn serve(server: &MockServer, route: &str, body: Vec<u8>, times: u64) {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
            .expect(times)
            .mount(server)
            .await;
    }

    fn files_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    #[tokio::test]
    async fn downloads_once_then_serves_from_memory() {
        let server = MockServer::start().await;
        serve(&server, "/cover/a", png(300, 300), 1).await;
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(dir.path(), 10 << 20, 10);
        let url = format!("{}/cover/a", server.uri());

        assert_eq!(cache.get_cached("a", 100), None);
        let first = cache.get("a", &url, 100).await.unwrap();
        assert_eq!((first.width, first.height), (100, 100));
        assert_eq!(first.rgba.len(), 100 * 100 * 4);
        assert_eq!(&first.rgba[..4], &[200, 10, 30, 255]);
        let second = cache.get("a", &url, 100).await.unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert!(cache.get_cached("a", 100).is_some());
        assert_eq!(cache.get_cached("a", 200), None);
        assert_eq!(files_in(dir.path()), vec![file_name("a", 100)]);
    }

    #[tokio::test]
    async fn reloads_from_disk_in_a_new_instance() {
        let server = MockServer::start().await;
        serve(&server, "/c", png(64, 64), 1).await;
        let dir = tempfile::tempdir().unwrap();
        let url = format!("{}/c", server.uri());
        cache(dir.path(), 10 << 20, 10)
            .get("k", &url, 64)
            .await
            .unwrap();

        let reopened = cache(dir.path(), 10 << 20, 10);
        assert!(reopened.disk_usage() > 0);
        assert_eq!(reopened.get_cached("k", 64), None);
        let image = reopened
            .get("k", "http://127.0.0.1:9/unreachable", 64)
            .await
            .unwrap();
        assert_eq!((image.width, image.height), (64, 64));
        assert!(reopened.get_cached("k", 64).is_some());
    }

    #[tokio::test]
    async fn resizes_to_fit_keeping_aspect_and_never_upscales() {
        let server = MockServer::start().await;
        serve(&server, "/wide", png(400, 200), 1).await;
        serve(&server, "/small", png(40, 30), 1).await;
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(dir.path(), 10 << 20, 10);

        let wide = cache
            .get("wide", &format!("{}/wide", server.uri()), 100)
            .await
            .unwrap();
        assert_eq!((wide.width, wide.height), (100, 50));
        let small = cache
            .get("small", &format!("{}/small", server.uri()), 100)
            .await
            .unwrap();
        assert_eq!((small.width, small.height), (40, 30));
    }

    #[tokio::test]
    async fn sizes_are_cached_separately() {
        let server = MockServer::start().await;
        serve(&server, "/c", png(300, 300), 2).await;
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(dir.path(), 10 << 20, 10);
        let url = format!("{}/c", server.uri());
        let small = cache.get("c", &url, 50).await.unwrap();
        let large = cache.get("c", &url, 150).await.unwrap();
        assert_eq!(small.width, 50);
        assert_eq!(large.width, 150);
        assert_eq!(files_in(dir.path()).len(), 2);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_requests_download_once() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/slow"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(png(120, 120))
                    .set_delay(Duration::from_millis(200)),
            )
            .expect(1)
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let cache = Arc::new(cache(dir.path(), 10 << 20, 0));
        let url = format!("{}/slow", server.uri());

        let tasks: Vec<_> = (0..16)
            .map(|_| {
                let cache = Arc::clone(&cache);
                let url = url.clone();
                tokio::spawn(async move { cache.get("slow", &url, 60).await })
            })
            .collect();
        let mut images = Vec::new();
        for task in tasks {
            images.push(task.await.unwrap().unwrap());
        }
        assert!(images.iter().all(|i| Arc::ptr_eq(i, &images[0])));
        assert!(lock(&cache.inner.inflight).is_empty());
    }

    #[tokio::test]
    async fn memory_lru_is_bounded() {
        let server = MockServer::start().await;
        let body = png(10, 10);
        for route in ["/1", "/2", "/3"] {
            serve(&server, route, body.clone(), 1).await;
        }
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(dir.path(), 10 << 20, 2);
        for key in ["1", "2", "3"] {
            cache
                .get(key, &format!("{}/{key}", server.uri()), 10)
                .await
                .unwrap();
        }
        assert_eq!(cache.get_cached("1", 10), None);
        assert!(cache.get_cached("2", 10).is_some());
        assert!(cache.get_cached("3", 10).is_some());

        let again = cache
            .get("1", "http://127.0.0.1:9/unused", 10)
            .await
            .unwrap();
        assert_eq!(again.width, 10);
        assert_eq!(cache.get_cached("2", 10), None);
    }

    #[tokio::test]
    async fn disk_is_bounded_by_evicting_least_recently_used() {
        let server = MockServer::start().await;
        let body = png(20, 20);
        let len = body.len() as u64;
        for route in ["/a", "/b", "/c"] {
            serve(&server, route, body.clone(), 1).await;
        }
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(dir.path(), 2 * len + 1, 0);
        let url = |key: &str| format!("{}/{key}", server.uri());

        cache.get("a", &url("a"), 20).await.unwrap();
        cache.get("b", &url("b"), 20).await.unwrap();
        cache
            .get("a", "http://127.0.0.1:9/unused", 20)
            .await
            .unwrap();
        cache.get("c", &url("c"), 20).await.unwrap();

        let mut expected = vec![file_name("a", 20), file_name("c", 20)];
        expected.sort();
        assert_eq!(files_in(dir.path()), expected);
        assert_eq!(cache.disk_usage(), 2 * len);
    }

    #[test]
    fn startup_trims_oldest_files_and_removes_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let now = SystemTime::now();
        for (i, name) in ["old", "mid", "new"].iter().enumerate() {
            let path = dir.path().join(name);
            std::fs::write(&path, vec![0u8; 100]).unwrap();
            let file = std::fs::File::options().append(true).open(&path).unwrap();
            file.set_modified(now - Duration::from_secs(1000 - i as u64 * 100))
                .unwrap();
        }
        std::fs::write(dir.path().join(".tmp-abc.part"), b"partial").unwrap();

        let cache = cache(dir.path(), 250, 1);
        assert_eq!(
            files_in(dir.path()),
            vec!["mid".to_string(), "new".to_string()]
        );
        assert_eq!(cache.disk_usage(), 200);
    }

    #[tokio::test]
    async fn http_errors_are_not_cached() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/c"))
            .respond_with(ResponseTemplate::new(404))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        serve(&server, "/c", png(8, 8), 1).await;
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(dir.path(), 10 << 20, 4);
        let url = format!("{}/c", server.uri());

        assert!(matches!(
            cache.get("c", &url, 8).await,
            Err(StorageError::Network(_))
        ));
        assert!(files_in(dir.path()).is_empty());
        assert_eq!(cache.get("c", &url, 8).await.unwrap().width, 8);
    }

    #[tokio::test]
    async fn undecodable_bodies_are_rejected_and_not_stored() {
        let server = MockServer::start().await;
        serve(&server, "/bad", b"<html>not an image</html>".to_vec(), 1).await;
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(dir.path(), 10 << 20, 4);
        assert!(matches!(
            cache.get("bad", &format!("{}/bad", server.uri()), 8).await,
            Err(StorageError::Invalid(_))
        ));
        assert!(files_in(dir.path()).is_empty());
        assert_eq!(cache.disk_usage(), 0);
    }

    #[tokio::test]
    async fn corrupt_disk_file_is_replaced_by_a_download() {
        let server = MockServer::start().await;
        serve(&server, "/c", png(16, 16), 1).await;
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(file_name("c", 16)), b"garbage").unwrap();
        let cache = cache(dir.path(), 10 << 20, 4);
        let image = cache
            .get("c", &format!("{}/c", server.uri()), 16)
            .await
            .unwrap();
        assert_eq!(image.width, 16);
        assert_eq!(
            std::fs::read(dir.path().join(file_name("c", 16))).unwrap(),
            png(16, 16)
        );
    }
}
