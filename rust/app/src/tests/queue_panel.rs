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
