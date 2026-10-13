use std::time::{Duration, Instant};

use feishin_api::MusicServer;
use feishin_core::model::{
    AlbumListQuery, ArtistListQuery, Paging, SongListQuery, SongSort, SortOrder,
};

use super::artist_albums::ArtistAlbums;
use super::{DemoLibrary, FakeServer};

const BIG_LIBRARY_SONGS: usize = 100_000;
const DEEP_PAGE_BUDGET: Duration = Duration::from_secs(10);

fn deep_paging(start_index: usize) -> Paging {
    Paging {
        start_index,
        limit: 100,
    }
}

#[test]
fn library_dimensions_scale_with_the_song_count() {
    let default_library = DemoLibrary::default();
    assert_eq!(default_library.song_count(), 20_000);
    assert_eq!(default_library.album_count(), 1_667);
    assert_eq!(default_library.artist_count(), 400);
    assert_eq!(DemoLibrary::with_songs(1_000).artist_count(), 400);
    let big_library = DemoLibrary::with_songs(BIG_LIBRARY_SONGS);
    assert_eq!(big_library.album_count(), 8_334);
    assert_eq!(big_library.artist_count(), 2_000);
    assert_eq!(big_library.album_song_range(8_333), 99_996..100_000);
    assert!(big_library.album_song_range(9_000).is_empty());
}

#[test]
fn every_album_is_indexed_under_its_artist() {
    let library = DemoLibrary::with_songs(BIG_LIBRARY_SONGS);
    let index = ArtistAlbums::index(library);
    let indexed: usize = (0..library.artist_count())
        .map(|artist| index.albums_of(artist).len())
        .sum();
    assert_eq!(indexed, library.album_count());
    assert!(
        index
            .albums_of(7)
            .iter()
            .all(|&album| library.album_artist(album) == 7)
    );
    assert!(index.albums_of(library.artist_count()).is_empty());
}

#[tokio::test]
async fn a_big_library_serves_deep_song_pages_quickly() {
    let server = FakeServer::new(BIG_LIBRARY_SONGS);
    let query = SongListQuery {
        sort: SongSort::Duration,
        order: SortOrder::Desc,
        ..SongListQuery::default()
    };
    let started = Instant::now();
    let deep = server.song_list(&query, deep_paging(99_000)).await;
    let tail = server.song_list(&query, deep_paging(99_950)).await;
    let elapsed = started.elapsed();
    assert!(elapsed < DEEP_PAGE_BUDGET, "deep pages took {elapsed:?}");
    let deep = deep.map(|page| (page.items.len(), page.total_count)).ok();
    assert_eq!(deep, Some((100, Some(BIG_LIBRARY_SONGS))));
    assert_eq!(tail.map(|page| page.items.len()).ok(), Some(50));
}

#[tokio::test]
async fn a_page_past_the_end_is_empty_but_keeps_the_total() {
    let server = FakeServer::new(BIG_LIBRARY_SONGS);
    let page = server
        .song_list(&SongListQuery::default(), deep_paging(usize::MAX))
        .await
        .ok();
    let page = page.map(|page| (page.items.len(), page.total_count));
    assert_eq!(page, Some((0, Some(BIG_LIBRARY_SONGS))));
}

#[tokio::test]
async fn a_big_library_serves_deep_album_and_artist_pages_quickly() {
    let server = FakeServer::new(BIG_LIBRARY_SONGS);
    let started = Instant::now();
    let albums = server
        .album_list(&AlbumListQuery::default(), deep_paging(8_300))
        .await;
    let artists = server
        .artist_list(&ArtistListQuery::default(), deep_paging(1_950))
        .await;
    let elapsed = started.elapsed();
    assert!(elapsed < DEEP_PAGE_BUDGET, "deep pages took {elapsed:?}");
    assert_eq!(albums.map(|page| page.items.len()).ok(), Some(34));
    assert_eq!(artists.map(|page| page.items.len()).ok(), Some(50));
}

#[tokio::test]
async fn a_saved_queue_restores_library_songs_and_unknown_ids() {
    let server = FakeServer::new(BIG_LIBRARY_SONGS);
    let saved: Vec<String> = ["song-99999", "elsewhere"].map(String::from).into();
    assert!(
        server
            .save_play_queue(&saved, Some("song-99999"), 7)
            .await
            .is_ok()
    );
    let restored = server.play_queue().await.ok().flatten();
    let songs = restored.map(|queue| {
        queue
            .songs
            .into_iter()
            .map(|song| (song.id, song.album_id))
            .collect::<Vec<_>>()
    });
    assert_eq!(
        songs,
        Some(vec![
            ("song-99999".to_owned(), Some("album-8333".to_owned())),
            ("elsewhere".to_owned(), None),
        ])
    );
}
