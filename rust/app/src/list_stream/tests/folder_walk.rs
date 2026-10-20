use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use feishin_api::{ApiError, MusicServer};
use feishin_core::model::{Folder, FolderId, Page, Paging, Song};

use crate::demo::FakeServer;
use crate::list_stream::folder_pages::FolderPages;
use crate::list_stream::folder_walk::{FolderSource, FolderWalk};
use crate::list_stream::source::PageFetcher;

struct FlakyFolders {
    server: FakeServer,
    calls: AtomicUsize,
    failing_calls: Vec<usize>,
}

impl FlakyFolders {
    fn new(song_count: usize, failing_calls: Vec<usize>) -> Self {
        Self {
            server: FakeServer::new(song_count),
            calls: AtomicUsize::new(0),
            failing_calls,
        }
    }

    fn call(&self) -> feishin_api::Result<()> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        match self.failing_calls.contains(&call) {
            true => Err(ApiError::Network(format!("call {call} dropped").into())),
            false => Ok(()),
        }
    }

    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl FolderSource for FlakyFolders {
    async fn child_folders(&self, folder: &FolderId) -> feishin_api::Result<Vec<Folder>> {
        self.call()?;
        self.server.subfolders(folder).await
    }

    async fn song_page(
        &self,
        folder: &FolderId,
        paging: Paging,
    ) -> feishin_api::Result<Page<Song>> {
        self.call()?;
        self.server.folder_song_page(folder, paging).await
    }
}

fn ids(songs: &[Song]) -> Vec<String> {
    songs.iter().map(|song| song.id.clone()).collect()
}

fn paging(start_index: usize, limit: usize) -> Paging {
    Paging { start_index, limit }
}

async fn album_folder_songs(server: &FakeServer, album_folder: &str) -> Vec<String> {
    let folder = FolderId::directory(album_folder);
    let page = server.folder_song_page(&folder, paging(0, 100)).await;
    page.map(|page| ids(&page.items)).unwrap_or_default()
}

async fn artist_with_two_albums(server: &FakeServer) -> (FolderId, Vec<String>) {
    let artists = server.subfolders(&FolderId::Root).await.unwrap();
    for artist in artists {
        let albums = server.subfolders(&artist.folder_id()).await.unwrap();
        if albums.len() < 2 {
            continue;
        }
        let mut expected = Vec::new();
        for album in &albums {
            expected.extend(album_folder_songs(server, &album.id).await);
        }
        return (artist.folder_id(), expected);
    }
    panic!("no artist with two albums");
}

#[tokio::test]
async fn a_walk_lists_each_album_folder_in_order_across_chunks() {
    let server = FakeServer::new(3_000);
    let (artist, expected) = artist_with_two_albums(&server).await;
    let mut walk = FolderWalk::new(artist);

    let mut walked = Vec::new();
    loop {
        let chunk = walk.next_songs(&server, 5).await.unwrap();
        assert!(chunk.len() <= 5);
        if chunk.is_empty() {
            break;
        }
        walked.extend(ids(&chunk));
    }

    assert_eq!(walked, expected);
    assert_eq!(walk.walked(), expected.len());
}

#[tokio::test]
async fn walking_the_root_reaches_every_song_once() {
    let server = FakeServer::new(1_300);
    let mut walk = FolderWalk::new(FolderId::Root);

    let mut seen = HashSet::new();
    loop {
        let chunk = walk.next_songs(&server, 500).await.unwrap();
        if chunk.is_empty() {
            break;
        }
        for song in chunk {
            assert!(seen.insert(song.id), "a song was walked twice");
        }
    }

    assert_eq!(seen.len(), 1_300);
}

#[tokio::test]
async fn folder_pages_continue_in_sequence_and_restart_when_asked_out_of_order() {
    let server = Arc::new(FakeServer::new(1_300));
    let pages = FolderPages::new(server.clone(), FolderId::Root);
    let mut expected_walk = FolderWalk::new(FolderId::Root);
    let expected = expected_walk
        .next_songs(server.as_ref(), 1_300)
        .await
        .unwrap();

    let first = pages.fetch(paging(0, 500)).await.unwrap();
    let second = pages.fetch(paging(500, 500)).await.unwrap();
    let again = pages.fetch(paging(250, 10)).await.unwrap();
    let last = pages.fetch(paging(1_000, 500)).await.unwrap();

    assert_eq!(ids(&first.items), ids(&expected[..500]));
    assert_eq!(ids(&second.items), ids(&expected[500..1_000]));
    assert_eq!(ids(&again.items), ids(&expected[250..260]));
    assert_eq!(last.items.len(), 300);
    assert_eq!(last.total_count, None);
}

#[tokio::test]
async fn songs_directly_in_a_folder_page_without_walking_subfolders() {
    let server = FakeServer::new(500);
    let page = crate::library::song_page(
        &server,
        &crate::list_stream::SongSource::Folder(FolderId::Root),
        paging(0, 100),
    )
    .await
    .unwrap();
    assert!(page.items.is_empty());
}

#[tokio::test]
async fn a_failed_request_resumes_the_walk_without_losing_or_repeating_songs() {
    let reliable = FakeServer::new(1_300);
    let expected = FolderWalk::new(FolderId::Root)
        .next_songs(&reliable, 1_300)
        .await
        .unwrap();
    let flaky = FlakyFolders::new(1_300, vec![2, 5, 9, 40, 41, 300]);
    let mut walk = FolderWalk::new(FolderId::Root);

    let mut walked = Vec::new();
    let mut failures = 0;
    loop {
        let before = walk.walked();
        match walk.next_songs(&flaky, 50).await {
            Ok(chunk) if chunk.is_empty() => break,
            Ok(chunk) => walked.extend(chunk),
            Err(_) => {
                failures += 1;
                assert_eq!(walk.walked(), before);
            }
        }
    }

    assert_eq!(failures, 6);
    assert_eq!(ids(&walked), ids(&expected));
}

#[tokio::test]
async fn asking_for_the_same_page_again_does_not_walk_again() {
    let source = Arc::new(FlakyFolders::new(1_300, Vec::new()));
    let pages = FolderPages::new(source.clone(), FolderId::Root);

    let first = pages.fetch(paging(0, 500)).await.unwrap();
    let calls_after_first = source.call_count();
    let again = pages.fetch(paging(0, 500)).await.unwrap();
    let next = pages.fetch(paging(500, 500)).await.unwrap();

    assert_eq!(ids(&again.items), ids(&first.items));
    assert!(calls_after_first > 0);
    assert!(source.call_count() > calls_after_first);
    assert_eq!(next.items.len(), 500);
    assert!(
        ids(&next.items)
            .iter()
            .all(|id| !ids(&first.items).contains(id))
    );
}

async fn fetch_retrying_once(
    pages: &FolderPages<FlakyFolders>,
    wanted: Paging,
) -> (Vec<String>, bool) {
    match pages.fetch(wanted).await {
        Ok(page) => (ids(&page.items), false),
        Err(_) => (ids(&pages.fetch(wanted).await.unwrap().items), true),
    }
}

#[tokio::test]
async fn failed_pages_are_fetched_again_from_where_the_walk_stopped() {
    let counting = Arc::new(FlakyFolders::new(1_300, Vec::new()));
    FolderPages::new(counting.clone(), FolderId::Root)
        .fetch(paging(0, 500))
        .await
        .unwrap();
    let first_page_calls = counting.call_count();
    let source = Arc::new(FlakyFolders::new(1_300, vec![10, first_page_calls + 10]));
    let pages = FolderPages::new(source, FolderId::Root);
    let reliable = FakeServer::new(1_300);
    let expected = FolderWalk::new(FolderId::Root)
        .next_songs(&reliable, 1_300)
        .await
        .unwrap();

    let mut walked = Vec::new();
    let mut failures = 0;
    for start_index in [0, 500, 1_000] {
        let (songs, failed) = fetch_retrying_once(&pages, paging(start_index, 500)).await;
        walked.extend(songs);
        failures += usize::from(failed);
    }

    assert_eq!(failures, 2);
    assert_eq!(walked, ids(&expected));
}
