use std::collections::HashSet;
use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_core::model::{FolderId, Paging, Song};

use crate::demo::FakeServer;
use crate::list_stream::folder_pages::FolderPages;
use crate::list_stream::folder_walk::FolderWalk;
use crate::list_stream::source::PageFetcher;

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
