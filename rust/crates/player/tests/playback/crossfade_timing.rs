use std::thread;
use std::time::{Duration, Instant};

use feishin_player::{MediaSource, Player, PlayerConfig, PlayerEvent};

use crate::support::{Recorder, has_error, player_with, position_of, sine_wave_url};

const POLL_INTERVAL: Duration = Duration::from_millis(20);
const POLL_TIMEOUT: Duration = Duration::from_secs(10);
const PLAYED_AFTER_PRELOAD_MS: u64 = 500;

fn crossfade_player(crossfade: Duration) -> (Player, std::sync::Arc<Recorder>) {
    player_with(PlayerConfig {
        crossfade,
        ..PlayerConfig::default()
    })
}

fn load_track(player: &Player, uid: u64, seconds: u32, hinted_seconds: u32) {
    let url = sine_wave_url(seconds);
    let source = MediaSource::new(uid, &url).with_duration_ms(u64::from(hinted_seconds) * 1_000);
    player.load_source(source, 0).unwrap();
}

fn preload_track(player: &Player, uid: u64, seconds: u32) {
    let url = sine_wave_url(seconds);
    let source = MediaSource::new(uid, &url).with_duration_ms(u64::from(seconds) * 1_000);
    player.preload_source(source).unwrap();
}

fn wait_until(description: &str, condition: impl Fn() -> bool) {
    let deadline = Instant::now() + POLL_TIMEOUT;
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting until {description}"
        );
        thread::sleep(POLL_INTERVAL);
    }
}

fn started(uid: u64) -> PlayerEvent {
    PlayerEvent::Started { uid }
}

fn ended(uid: u64) -> PlayerEvent {
    PlayerEvent::TrackEnded { uid }
}

#[test]
fn next_track_is_only_loaded_shortly_before_the_fade() {
    let (player, recorder) = crossfade_player(Duration::from_secs(1));
    load_track(&player, 1, 40, 40);
    recorder.wait_for_event(started(1));
    let event_count_before = recorder.event_count();
    preload_track(&player, 2, 2);
    recorder.wait_after(event_count_before, "positions after the preload", |event| {
        position_of(event).is_some_and(|position| position >= PLAYED_AFTER_PRELOAD_MS)
    });
    assert!(!player.holds_next_track());

    player.seek(37_000).unwrap();
    wait_until("the next track is loaded", || player.holds_next_track());
    recorder.wait_for_event(PlayerEvent::Idle);
    assert_eq!(
        recorder.milestones(),
        vec![started(1), started(2), ended(2), PlayerEvent::Idle]
    );
}

#[test]
fn track_ending_before_its_expected_length_still_plays_the_next_one() {
    let (player, recorder) = crossfade_player(Duration::from_secs(1));
    load_track(&player, 1, 2, 60);
    recorder.wait_for_event(started(1));
    preload_track(&player, 2, 1);
    assert!(!player.holds_next_track());
    recorder.wait_for_event(PlayerEvent::Idle);

    assert_eq!(
        recorder.milestones(),
        vec![
            started(1),
            ended(1),
            started(2),
            ended(2),
            PlayerEvent::Idle
        ]
    );
}

#[test]
fn turning_crossfade_off_during_a_fade_lets_it_finish() {
    let (player, recorder) = crossfade_player(Duration::from_secs(2));
    load_track(&player, 1, 3, 3);
    preload_track(&player, 2, 10);
    recorder.wait_for_event(started(2));
    player.set_crossfade(Duration::ZERO).unwrap();
    assert!(player.is_crossfading());

    wait_until("the fade finishes", || !player.is_crossfading());
    assert_eq!(recorder.milestones(), vec![started(1), started(2)]);
    assert!(!has_error(&recorder.snapshot()));
}
