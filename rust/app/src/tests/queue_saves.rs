use std::time::Duration;

use feishin_core::queue::Queue;
use feishin_player::PlayerEvent;
use feishin_storage::queue_store::{self, StoredQueue};

use super::harness::{
    Harness, settle, start_harness, start_harness_with, test_settings, test_songs,
};
use crate::controller::Command;
use crate::store::Store;

const QUEUE_SAVE_DELAY: Duration = Duration::from_secs(10);
const PLAYBACK_SAVE_DELAY: Duration = Duration::from_secs(2);
const WRITE_POLLS: usize = 500;

async fn advance_and_handle(harness: &mut Harness, duration: Duration) {
    tokio::time::advance(duration).await;
    settle().await;
    while let Ok(command) = harness.commands.try_recv() {
        harness.controller.handle(command);
    }
}

async fn wait_for_saved(harness: &Harness, accept: impl Fn(&StoredQueue) -> bool) -> bool {
    for _ in 0..WRITE_POLLS {
        settle().await;
        if harness
            .store
            .load_queue()
            .ok()
            .flatten()
            .as_ref()
            .is_some_and(&accept)
        {
            return true;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    false
}

fn saved_ids(stored: &StoredQueue) -> Vec<String> {
    let queue = Queue::from_snapshot(stored.snapshot.clone());
    queue
        .display_order()
        .iter()
        .map(|item| item.track.song_id.to_string())
        .collect()
}

#[tokio::test(start_paused = true)]
async fn queue_is_saved_ten_seconds_after_the_last_structural_change() {
    let mut harness = start_harness(test_settings(false, false));
    harness.controller.play_now(test_songs(3, 100_000), "s0");
    advance_and_handle(&mut harness, Duration::from_secs(6)).await;
    harness
        .controller
        .handle(Command::AddLast(test_songs(5, 100_000).split_off(3)));
    advance_and_handle(&mut harness, Duration::from_secs(5)).await;
    std::thread::sleep(Duration::from_millis(20));
    settle().await;
    assert!(harness.store.load_queue().ok().flatten().is_none());

    advance_and_handle(&mut harness, QUEUE_SAVE_DELAY).await;
    let saved = wait_for_saved(&harness, |stored| saved_ids(stored).len() == 5).await;
    assert!(saved, "the queue is saved once the edits settle");
}

#[tokio::test(start_paused = true)]
async fn position_changes_rewrite_only_the_small_playback_file() {
    let mut harness = start_harness(test_settings(false, false));
    harness.controller.play_now(test_songs(4, 100_000), "s1");
    harness.start_current();
    advance_and_handle(&mut harness, QUEUE_SAVE_DELAY).await;
    assert!(wait_for_saved(&harness, |stored| stored.snapshot.items.len() == 4).await);
    let queue_file = harness.temp_dir.path().join(queue_store::QUEUE_FILE);
    let saved_queue = std::fs::read(&queue_file).unwrap_or_default();

    harness.controller.handle(Command::Next);
    harness.controller.handle(Command::Seek(42_000));
    advance_and_handle(&mut harness, PLAYBACK_SAVE_DELAY).await;
    let moved = wait_for_saved(&harness, |stored| stored.position_ms == 42_000).await;
    assert!(moved, "the position is saved without the queue changing");
    assert_eq!(std::fs::read(&queue_file).unwrap_or_default(), saved_queue);

    let saved_settings = harness.store.load_settings().unwrap_or_default();
    let restored = start_harness_with(saved_settings, 100, Some(harness.temp_dir));
    assert_eq!(restored.current_id().as_deref(), Some("s2"));
}

#[tokio::test]
async fn shutdown_saves_pending_changes() {
    let mut harness = start_harness(test_settings(false, false));
    harness.controller.play_now(test_songs(6, 100_000), "s4");
    harness.start_current();
    harness.player(PlayerEvent::Position {
        position_ms: 7_000,
        duration_ms: None,
    });
    assert!(harness.shutdown().await.is_ok());
    let stored = harness.store.load_queue().ok().flatten();
    assert_eq!(stored.as_ref().map(saved_ids).map(|ids| ids.len()), Some(6));
    assert_eq!(stored.map(|stored| stored.position_ms), Some(7_000));
}
