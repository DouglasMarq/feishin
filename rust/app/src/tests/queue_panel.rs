use feishin_core::queue::Uid;

use super::harness::{start_harness, test_settings, test_songs};
use crate::controller::Command;

#[tokio::test]
async fn queue_panel_reorders_and_shows_play_order() {
    let mut harness = start_harness(test_settings(false, false));
    harness.controller.play_now(test_songs(3, 1000), "s0");
    let uids: Vec<Uid> = ["s0", "s1", "s2"]
        .iter()
        .map(|song_id| harness.uid_of(song_id).unwrap_or(Uid::MAX))
        .collect();
    harness.controller.handle(Command::Move {
        uids: vec![uids[2]],
        before: Some(uids[0]),
    });
    assert_eq!(
        harness.ui.last_queue(),
        Some(vec![uids[2], uids[0], uids[1]])
    );

    harness.controller.handle(Command::ToggleShuffle);
    let shown = harness.ui.last_queue().unwrap_or_default();
    let order: Vec<Uid> = harness
        .controller
        .queue()
        .display_order()
        .iter()
        .map(|item| item.uid)
        .collect();
    assert_eq!(shown, order);
    assert_eq!(shown.first(), harness.current_uid().as_ref());
}

#[tokio::test]
async fn track_changes_republish_the_same_queue_layout() {
    let mut harness = start_harness(test_settings(false, false));
    harness.controller.play_now(test_songs(4, 1000), "s1");
    let Some(before) = harness.ui.last_queue_view() else {
        panic!("expected a published queue");
    };
    assert_eq!(before.current_position(), Some(1));
    harness.controller.handle(Command::Next);
    let Some(after) = harness.ui.last_queue_view() else {
        panic!("expected a published queue");
    };
    assert!(after.shares_layout_with(&before));
    assert_eq!(after.current_position(), Some(2));
    assert_eq!(after.current().map(|item| item.uid), harness.current_uid());
}

#[tokio::test]
async fn queue_edits_publish_a_new_layout_with_the_current_position() {
    let mut harness = start_harness(test_settings(false, false));
    harness.controller.play_now(test_songs(4, 1000), "s2");
    let removed = harness.uid_of("s0").unwrap_or(Uid::MAX);
    let before = harness.ui.last_queue_view();
    harness.controller.handle(Command::Remove(vec![removed]));
    let Some(after) = harness.ui.last_queue_view() else {
        panic!("expected a published queue");
    };
    assert!(before.is_some_and(|before| !after.shares_layout_with(&before)));
    assert_eq!(after.len(), 3);
    assert_eq!(after.current_position(), Some(1));
    assert_eq!(after.current().map(|item| item.uid), harness.current_uid());
}
