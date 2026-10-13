use feishin_core::queue::Uid;
use feishin_player::PlayerEvent;

use super::harness::{AudioCall, start_harness, test_settings, test_songs};
use crate::controller::Command;
use crate::sleep_timer::{SleepMode, SleepStatus};

#[tokio::test]
async fn timed_sleep_timer_counts_listening_time_and_pauses() {
    let mut harness = start_harness(test_settings(false, false));
    harness.controller.play_now(test_songs(2, 3_600_000), "s0");
    harness.start_current();
    harness
        .controller
        .handle(Command::SleepTimer(Some(SleepMode::Minutes(15))));
    harness.audio.take();

    harness.controller.handle(Command::SleepTick {
        elapsed_ms: 14 * 60_000,
    });
    harness.controller.handle(Command::TogglePause);
    harness.controller.handle(Command::SleepTick {
        elapsed_ms: 5 * 60_000,
    });
    assert_eq!(
        harness.controller.sleep_status(),
        Some(SleepStatus::Remaining {
            remaining_ms: 60_000
        })
    );
    harness.controller.handle(Command::TogglePause);
    assert_eq!(harness.audio.take(), [AudioCall::Pause, AudioCall::Play]);

    harness
        .controller
        .handle(Command::SleepTick { elapsed_ms: 60_000 });
    assert_eq!(harness.audio.take(), [AudioCall::Pause]);
    assert!(!harness.controller.is_playing());
    assert_eq!(harness.controller.sleep_status(), None);
}

#[tokio::test]
async fn end_of_track_sleep_timer_stops_on_the_next_song() {
    let mut harness = start_harness(test_settings(true, false));
    harness.controller.play_now(test_songs(3, 1000), "s0");
    let first = harness.start_current();
    let second = harness.uid_of("s1").unwrap_or(Uid::MAX);
    assert_eq!(
        harness.audio.take().last(),
        Some(&AudioCall::Preload(second))
    );

    harness
        .controller
        .handle(Command::SleepTimer(Some(SleepMode::EndOfTrack)));
    assert_eq!(harness.audio.take(), [AudioCall::ClearNext]);

    harness.player(PlayerEvent::TrackEnded { uid: first });
    assert_eq!(harness.audio.take(), [AudioCall::Stop]);
    assert!(!harness.controller.is_playing());
    assert_eq!(harness.current_uid(), Some(second));
    assert_eq!(harness.controller.sleep_status(), None);

    harness.controller.handle(Command::TogglePause);
    assert_eq!(harness.audio.take(), [AudioCall::Load(second, 0)]);
}

#[tokio::test]
async fn cancelling_end_of_track_restores_the_preload() {
    let mut harness = start_harness(test_settings(true, false));
    harness.controller.play_now(test_songs(2, 1000), "s0");
    harness.start_current();
    harness
        .controller
        .handle(Command::SleepTimer(Some(SleepMode::EndOfTrack)));
    harness.audio.take();
    harness.controller.handle(Command::SleepTimer(None));
    let second = harness.uid_of("s1").unwrap_or(Uid::MAX);
    assert_eq!(harness.audio.take(), [AudioCall::Preload(second)]);
}
