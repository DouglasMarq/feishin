use feishin_player::PlayerEvent;

use super::harness::{Harness, settle, start_harness, test_settings, test_songs};
use crate::demo::ScrobbleCall;

const SAMPLE_INTERVAL_MS: u64 = 250;

fn feed_positions(harness: &mut Harness, samples: std::ops::RangeInclusive<u64>, duration_ms: u64) {
    for sample in samples {
        harness.player(PlayerEvent::Position {
            position_ms: sample * SAMPLE_INTERVAL_MS,
            duration_ms: Some(duration_ms),
        });
    }
}

#[tokio::test]
async fn scrobbles_now_playing_on_start_and_submits_at_half() {
    let mut harness = start_harness(test_settings(false, true));
    harness.controller.play_now(test_songs(1, 60_000), "s0");
    harness.start_current();
    settle().await;
    assert_eq!(
        harness.server.scrobbles(),
        [ScrobbleCall {
            song_id: "s0".into(),
            submission: false,
            timestamp_ms: None
        }]
    );

    feed_positions(&mut harness, 0..=(29_000 / SAMPLE_INTERVAL_MS), 60_000);
    settle().await;
    assert_eq!(harness.server.scrobbles().len(), 1);

    feed_positions(
        &mut harness,
        (29_000 / SAMPLE_INTERVAL_MS)..=(40_000 / SAMPLE_INTERVAL_MS),
        60_000,
    );
    settle().await;
    let calls = harness.server.scrobbles();
    assert_eq!(calls.len(), 2);
    assert!(calls[1].submission && calls[1].song_id == "s0");
}

#[tokio::test]
async fn scrobbling_disabled_sends_nothing() {
    let mut harness = start_harness(test_settings(false, false));
    harness.controller.play_now(test_songs(1, 10_000), "s0");
    harness.start_current();
    feed_positions(&mut harness, 0..=39, 10_000);
    settle().await;
    assert!(harness.server.scrobbles().is_empty());
}
