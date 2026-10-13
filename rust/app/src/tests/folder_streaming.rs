use feishin_api::MusicServer;
use feishin_core::model::FolderId;

use super::harness::{Harness, start_harness_with, test_settings, test_songs};
use crate::controller::Command;
use crate::list_stream::{FolderWalk, ListRow, Placement, RowRanges, SongSource};

async fn walked_ids(harness: &Harness, root: FolderId) -> Vec<String> {
    let mut walk = FolderWalk::new(root);
    let songs = walk.next_songs(harness.server.as_ref(), 100_000).await;
    songs
        .map(|songs| songs.into_iter().map(|song| song.id).collect())
        .unwrap_or_default()
}

async fn artist_with_songs(harness: &Harness) -> (FolderId, Vec<String>) {
    let artists = harness.server.subfolders(&FolderId::Root).await.unwrap();
    for artist in artists {
        let songs = walked_ids(harness, artist.folder_id()).await;
        if !songs.is_empty() {
            return (artist.folder_id(), songs);
        }
    }
    panic!("no artist folder has songs");
}

#[tokio::test]
async fn playing_a_folder_streams_its_whole_tree_into_the_queue() {
    let mut harness = start_harness_with(test_settings(false, false), 1_234, None);
    let expected = walked_ids(&harness, FolderId::Root).await;
    harness.controller.handle(Command::PlayFromList {
        source: SongSource::FolderTree(FolderId::Root),
        song_id: None,
        row_index: ListRow(0),
    });
    harness.handle_next_command().await;
    assert_eq!(harness.controller.queue().len(), 500);
    assert_eq!(harness.current_id().as_ref(), expected.first());

    harness.finish_streams().await;

    assert_eq!(expected.len(), 1_234);
    assert_eq!(harness.queued_ids(), expected);
}

#[tokio::test]
async fn a_folder_added_to_the_queue_lands_after_the_current_songs() {
    let mut harness = start_harness_with(test_settings(false, false), 600, None);
    let (artist, expected) = artist_with_songs(&harness).await;
    harness.controller.play_now(test_songs(2, 1000), "s0");

    harness.controller.handle(Command::EnqueueFromList {
        source: SongSource::FolderTree(artist),
        ranges: vec![RowRanges::WHOLE_LIST],
        placement: Placement::Last,
    });
    harness.finish_streams().await;

    let queued = harness.queued_ids();
    assert_eq!(queued[..2], ["s0", "s1"]);
    assert!(!expected.is_empty());
    assert_eq!(queued[2..], expected[..]);
}
