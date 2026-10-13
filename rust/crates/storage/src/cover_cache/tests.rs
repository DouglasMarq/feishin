use std::io::Cursor;
use std::path::Path;
use std::time::{Duration, SystemTime};

use image::{ImageFormat, Rgba, RgbaImage};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::image_file::cache_file_name;
use super::*;
use crate::StorageError;

fn png(width: u32, height: u32) -> Vec<u8> {
    let image = RgbaImage::from_pixel(width, height, Rgba([200, 10, 30, 255]));
    let mut encoded = Cursor::new(Vec::new());
    image.write_to(&mut encoded, ImageFormat::Png).unwrap();
    encoded.into_inner()
}

fn config(directory: &Path, max_disk_bytes: u64, max_memory_items: usize) -> CoverCacheConfig {
    CoverCacheConfig {
        dir: directory.to_path_buf(),
        max_disk_bytes,
        max_memory_items,
    }
}

fn cache(directory: &Path, max_disk_bytes: u64, max_memory_items: usize) -> CoverCache {
    CoverCache::new(
        config(directory, max_disk_bytes, max_memory_items),
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

fn files_in(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn downloads_once_then_serves_from_memory() {
    let server = MockServer::start().await;
    serve(&server, "/cover/a", png(300, 300), 1).await;
    let directory = tempfile::tempdir().unwrap();
    let cache = cache(directory.path(), 10 << 20, 10);
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
    assert_eq!(files_in(directory.path()), vec![cache_file_name("a", 100)]);
}

#[tokio::test]
async fn reloads_from_disk_in_a_new_instance() {
    let server = MockServer::start().await;
    serve(&server, "/c", png(64, 64), 1).await;
    let directory = tempfile::tempdir().unwrap();
    let url = format!("{}/c", server.uri());
    cache(directory.path(), 10 << 20, 10)
        .get("k", &url, 64)
        .await
        .unwrap();

    let reopened = cache(directory.path(), 10 << 20, 10);
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
    let directory = tempfile::tempdir().unwrap();
    let cache = cache(directory.path(), 10 << 20, 10);

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
    let directory = tempfile::tempdir().unwrap();
    let cache = cache(directory.path(), 10 << 20, 10);
    let url = format!("{}/c", server.uri());
    let small = cache.get("c", &url, 50).await.unwrap();
    let large = cache.get("c", &url, 150).await.unwrap();
    assert_eq!(small.width, 50);
    assert_eq!(large.width, 150);
    assert_eq!(files_in(directory.path()).len(), 2);
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
    let directory = tempfile::tempdir().unwrap();
    let cache = Arc::new(cache(directory.path(), 10 << 20, 0));
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
    assert!(images.iter().all(|image| Arc::ptr_eq(image, &images[0])));
    assert!(lock(&cache.inner.inflight).is_empty());
}

#[tokio::test]
async fn memory_lru_is_bounded() {
    let server = MockServer::start().await;
    let body = png(10, 10);
    for route in ["/1", "/2", "/3"] {
        serve(&server, route, body.clone(), 1).await;
    }
    let directory = tempfile::tempdir().unwrap();
    let cache = cache(directory.path(), 10 << 20, 2);
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
    let body_length = body.len() as u64;
    for route in ["/a", "/b", "/c"] {
        serve(&server, route, body.clone(), 1).await;
    }
    let directory = tempfile::tempdir().unwrap();
    let cache = cache(directory.path(), 2 * body_length + 1, 0);
    let url = |key: &str| format!("{}/{key}", server.uri());

    cache.get("a", &url("a"), 20).await.unwrap();
    cache.get("b", &url("b"), 20).await.unwrap();
    cache
        .get("a", "http://127.0.0.1:9/unused", 20)
        .await
        .unwrap();
    cache.get("c", &url("c"), 20).await.unwrap();

    let mut expected = vec![cache_file_name("a", 20), cache_file_name("c", 20)];
    expected.sort();
    assert_eq!(files_in(directory.path()), expected);
    assert_eq!(cache.disk_usage(), 2 * body_length);
}

#[test]
fn startup_trims_oldest_files_and_removes_temp_files() {
    let directory = tempfile::tempdir().unwrap();
    let now = SystemTime::now();
    for (age_step, name) in ["old", "mid", "new"].iter().enumerate() {
        let path = directory.path().join(name);
        std::fs::write(&path, vec![0u8; 100]).unwrap();
        let file = std::fs::File::options().append(true).open(&path).unwrap();
        file.set_modified(now - Duration::from_secs(1000 - age_step as u64 * 100))
            .unwrap();
    }
    std::fs::write(directory.path().join(".tmp-abc.part"), b"partial").unwrap();

    let cache = cache(directory.path(), 250, 1);
    assert_eq!(
        files_in(directory.path()),
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
    let directory = tempfile::tempdir().unwrap();
    let cache = cache(directory.path(), 10 << 20, 4);
    let url = format!("{}/c", server.uri());

    assert!(matches!(
        cache.get("c", &url, 8).await,
        Err(StorageError::Network(_))
    ));
    assert!(files_in(directory.path()).is_empty());
    assert_eq!(cache.get("c", &url, 8).await.unwrap().width, 8);
}

#[tokio::test]
async fn undecodable_bodies_are_rejected_and_not_stored() {
    let server = MockServer::start().await;
    serve(&server, "/bad", b"<html>not an image</html>".to_vec(), 1).await;
    let directory = tempfile::tempdir().unwrap();
    let cache = cache(directory.path(), 10 << 20, 4);
    assert!(matches!(
        cache.get("bad", &format!("{}/bad", server.uri()), 8).await,
        Err(StorageError::Invalid(_))
    ));
    assert!(files_in(directory.path()).is_empty());
    assert_eq!(cache.disk_usage(), 0);
}

#[tokio::test]
async fn corrupt_disk_file_is_replaced_by_a_download() {
    let server = MockServer::start().await;
    serve(&server, "/c", png(16, 16), 1).await;
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join(cache_file_name("c", 16)), b"garbage").unwrap();
    let cache = cache(directory.path(), 10 << 20, 4);
    let image = cache
        .get("c", &format!("{}/c", server.uri()), 16)
        .await
        .unwrap();
    assert_eq!(image.width, 16);
    assert_eq!(
        std::fs::read(directory.path().join(cache_file_name("c", 16))).unwrap(),
        png(16, 16)
    );
}
