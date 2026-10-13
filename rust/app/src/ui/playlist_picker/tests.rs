use std::cell::RefCell;
use std::collections::HashSet;

use feishin_api::MusicServer;
use feishin_core::model::{Paging, SongListQuery};

use super::picker::{PickedSongs, Picker};
use super::streamed::{add_listed_songs, existing_song_ids};
use crate::demo::FakeServer;
use crate::list_stream::SongSource;
use crate::ui::tracks::{ListedSongs, SelectionRanges};

async fn library_ids(server: &FakeServer, start_index: usize, limit: usize) -> Vec<String> {
    let paging = Paging { start_index, limit };
    server
        .song_list(&SongListQuery::default(), paging)
        .await
        .map(|page| page.items.into_iter().map(|song| song.id).collect())
        .unwrap_or_default()
}

async fn playlist_ids(server: &FakeServer, playlist_id: &str) -> Vec<String> {
    server
        .playlist_songs(playlist_id)
        .await
        .map(|songs| songs.into_iter().map(|song| song.id).collect())
        .unwrap_or_default()
}

fn library_rows(rows: std::ops::Range<usize>) -> ListedSongs {
    ListedSongs {
        source: SongSource::Library(SongListQuery::default()),
        ranges: SelectionRanges::from_ranges(vec![rows]),
    }
}

#[tokio::test]
async fn listed_songs_are_added_chunk_by_chunk_skipping_existing_ones() {
    let server = FakeServer::new(3000);
    let first_rows = library_ids(&server, 0, 1200).await;
    let already_there = vec![first_rows[700].clone(), first_rows[3].clone()];
    let playlist = server.create_playlist("Mine", &already_there).await;
    let Ok(playlist) = playlist else {
        panic!("create failed");
    };

    let existing = existing_song_ids(&server, &playlist.id).await;
    let existing = existing.unwrap_or_default();
    assert_eq!(
        existing,
        already_there.iter().cloned().collect::<HashSet<_>>()
    );

    let progress = RefCell::new(Vec::new());
    let report = |read| progress.borrow_mut().push(read);
    let listed = library_rows(0..1200);
    let outcome = add_listed_songs(&server, &playlist.id, listed, &existing, report).await;

    assert!(outcome.failure.is_none());
    assert_eq!((outcome.read, outcome.added), (1200, 1198));
    assert_eq!(progress.into_inner(), [500, 1000, 1200]);
    let added: Vec<String> = first_rows
        .into_iter()
        .filter(|song_id| !already_there.contains(song_id))
        .collect();
    let mut expected = already_there;
    expected.extend(added);
    assert_eq!(playlist_ids(&server, &playlist.id).await, expected);
}

#[tokio::test]
async fn adding_to_a_missing_playlist_reports_the_failure() {
    let server = FakeServer::new(100);
    assert!(existing_song_ids(&server, "missing").await.is_err());
    let outcome = add_listed_songs(
        &server,
        "missing",
        library_rows(0..10),
        &HashSet::new(),
        |_| {},
    )
    .await;
    assert!(outcome.failure.is_some());
    assert_eq!(outcome.added, 0);
}

#[test]
fn the_picker_keeps_listed_selections_until_a_request_starts() {
    let mut picker = Picker::default();
    let listed = library_rows(0..100_000);
    let generation = picker.restart(PickedSongs::Listed(listed.clone()));
    let Some(pending) = picker.start_request() else {
        panic!("a listed selection can be added");
    };
    assert_eq!(pending.generation, generation);
    assert_eq!(pending.songs, PickedSongs::Listed(listed));
    assert_eq!(pending.songs.song_count(), 100_000);
    assert!(picker.start_request().is_none());

    picker.restart(PickedSongs::default());
    assert!(picker.start_request().is_none());
}
