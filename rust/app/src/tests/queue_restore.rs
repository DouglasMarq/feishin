use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_core::queue::Queue;
use tokio::sync::mpsc;

use super::harness::{Harness, settle, start_harness, test_settings, test_songs};
use crate::controller::{Command, UiUpdate, load_saved_queue};
use crate::store::{JsonStore, Store};

fn saved_store(song_count: usize, current: &str) -> (tempfile::TempDir, Arc<JsonStore>) {
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(JsonStore::new(directory.path()));
    let mut queue = Queue::with_seed(5);
    queue.play_now(test_songs(song_count, 100_000), Some(current));
    let encoded = feishin_storage::queue_store::encode(&queue.snapshot_ref(), 3_210).unwrap();
    store.save_queue(&encoded).unwrap();
    (directory, store)
}

async fn deliver_saved_queue(harness: &mut Harness, store: Arc<dyn Store>) {
    let (sender, mut receiver) = mpsc::unbounded_channel();
    load_saved_queue(store, sender).await;
    let command = receiver.recv().await;
    assert!(matches!(command, Some(Command::Persistence(_))));
    if let Some(command) = command {
        harness.controller.handle(command);
    }
}

fn awaiting_harness() -> Harness {
    let mut harness = start_harness(test_settings(false, false));
    harness.controller.expect_saved_queue();
    harness
}

#[tokio::test]
async fn saved_queue_arriving_after_connect_is_restored_without_asking_the_server() {
    let (_directory, store) = saved_store(4, "s2");
    let mut harness = awaiting_harness();
    let seeded = harness
        .server
        .save_play_queue(&["song-1".to_owned()], None, 0)
        .await;
    assert!(seeded.is_ok());
    harness.connect();
    harness.ui.clear();

    deliver_saved_queue(&mut harness, store).await;
    assert_eq!(harness.queued_ids(), ["s0", "s1", "s2", "s3"]);
    assert_eq!(harness.current_id().as_deref(), Some("s2"));
    assert!(harness.ui.any(
        |update| matches!(update, UiUpdate::NowPlaying(Some(track)) if track.song_id == "s2")
    ));
    assert!(harness.audio.take().is_empty());
    settle().await;
    assert!(harness.commands.try_recv().is_err());
}

#[tokio::test]
async fn without_a_saved_queue_the_server_queue_is_fetched_once_loading_ends() {
    let empty_directory = tempfile::tempdir().unwrap();
    let mut harness = awaiting_harness();
    let seeded = harness
        .server
        .save_play_queue(
            &["song-3".to_owned(), "song-4".to_owned()],
            Some("song-4"),
            0,
        )
        .await;
    assert!(seeded.is_ok());
    harness.connect();
    settle().await;
    assert!(harness.commands.try_recv().is_err());

    deliver_saved_queue(
        &mut harness,
        Arc::new(JsonStore::new(empty_directory.path())),
    )
    .await;
    harness.handle_next_command().await;
    assert_eq!(harness.queued_ids(), ["song-3", "song-4"]);
}

#[tokio::test]
async fn a_corrupt_saved_queue_starts_empty() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory
        .path()
        .join(feishin_storage::queue_store::QUEUE_FILE);
    std::fs::write(file, b"FSQ1 not a queue").unwrap();
    let mut harness = awaiting_harness();
    deliver_saved_queue(&mut harness, Arc::new(JsonStore::new(directory.path()))).await;
    assert!(harness.controller.queue().is_empty());
    harness.controller.play_now(test_songs(2, 100_000), "s1");
    assert_eq!(harness.current_id().as_deref(), Some("s1"));
}

#[tokio::test]
async fn playing_before_the_saved_queue_arrives_keeps_the_new_queue() {
    let (_directory, store) = saved_store(4, "s2");
    let mut harness = awaiting_harness();
    harness.controller.handle(Command::PlaySongs {
        songs: test_songs(2, 100_000),
        song_id: Some("s1".into()),
    });
    deliver_saved_queue(&mut harness, store).await;
    assert_eq!(harness.queued_ids(), ["s0", "s1"]);
    assert_eq!(harness.current_id().as_deref(), Some("s1"));
}

#[tokio::test]
async fn quitting_while_the_saved_queue_loads_keeps_it_on_disk() {
    let mut harness = awaiting_harness();
    let mut queue = Queue::with_seed(5);
    queue.play_now(test_songs(3, 100_000), Some("s1"));
    let encoded = feishin_storage::queue_store::encode(&queue.snapshot_ref(), 0).unwrap();
    harness.store.save_queue(&encoded).unwrap();
    harness.connect();

    assert!(harness.shutdown().await.is_ok());
    let stored = harness.store.load_queue().ok().flatten();
    assert_eq!(stored.map(|stored| stored.snapshot.items.len()), Some(3));
    assert_eq!(harness.server.saved_play_queue().0, 0);
}
