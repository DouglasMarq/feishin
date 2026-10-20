use std::sync::{Arc, OnceLock, Weak};
use std::thread;
use std::time::Duration;

use feishin_player::{Player, PlayerConfig, PlayerEvent, ReplayGainMode};

use crate::support::{Recorder, has_error, player, player_with, position_of, sine_wave_url};

const POSITION_WINDOW_MS: u64 = 1_600;

#[test]
fn load_plays_to_natural_end() {
    let (player, recorder) = player();
    player.load(1, &sine_wave_url(1), 0).unwrap();
    recorder.wait_for_event(PlayerEvent::Idle);

    assert_eq!(
        recorder.milestones(),
        vec![
            PlayerEvent::Started { uid: 1 },
            PlayerEvent::TrackEnded { uid: 1 },
            PlayerEvent::Idle,
        ]
    );
    let positions = recorder.positions();
    assert!(positions.len() >= 3, "{positions:?}");
    assert!(
        positions.windows(2).all(|pair| pair[0].0 <= pair[1].0),
        "{positions:?}"
    );
    let duration = positions
        .iter()
        .find_map(|(_, duration_ms)| *duration_ms)
        .unwrap();
    assert!((900..=1100).contains(&duration), "{duration}");
}

#[test]
fn position_events_arrive_about_every_250ms() {
    let (player, recorder) = player();
    player.load(1, &sine_wave_url(3), 0).unwrap();
    recorder.wait_for_event(PlayerEvent::Started { uid: 1 });
    let event_count_before = recorder.event_count();
    recorder.wait_after(event_count_before, "1.6 s of positions", |event| {
        position_of(event).is_some_and(|position| position >= POSITION_WINDOW_MS)
    });
    let count = recorder.snapshot()[event_count_before..]
        .iter()
        .filter(|event| matches!(event, PlayerEvent::Position { .. }))
        .count();
    assert!((4..=9).contains(&count), "{count} positions in 1.6 s");
    assert!(player.position_ms().is_some());
}

#[test]
fn load_starts_at_offset() {
    let (player, recorder) = player();
    player.load(1, &sine_wave_url(10), 4000).unwrap();
    recorder.wait_for_event(PlayerEvent::Started { uid: 1 });
    recorder.wait_for("position", |event| {
        matches!(event, PlayerEvent::Position { .. })
    });
    assert!(
        recorder
            .positions()
            .iter()
            .all(|(position_ms, _)| *position_ms >= 4000),
        "{:?}",
        recorder.positions()
    );
}

fn pause_and_assert_position_freezes(player: &Player, recorder: &Recorder) -> usize {
    player.pause().unwrap();
    recorder.wait_for_event(PlayerEvent::Paused);
    thread::sleep(Duration::from_millis(200));
    let event_count_before = recorder.event_count();
    let paused_at = player.position_ms().unwrap();
    thread::sleep(Duration::from_millis(700));
    let after_pause: Vec<_> = recorder.snapshot()[event_count_before..].to_vec();
    assert!(
        !after_pause
            .iter()
            .any(|event| matches!(event, PlayerEvent::Position { .. })),
        "{after_pause:?}"
    );
    assert_eq!(player.position_ms(), Some(paused_at));
    event_count_before
}

fn toggle_pause_and_wait_for(player: &Player, recorder: &Recorder, expected: PlayerEvent) {
    let event_count_before = recorder.event_count();
    player.toggle_pause().unwrap();
    recorder.wait_for_event_after(event_count_before, expected);
}

#[test]
fn pause_and_resume() {
    let (player, recorder) = player();
    player.load(1, &sine_wave_url(10), 0).unwrap();
    recorder.wait_for(
        "playback underway",
        |event| matches!(event, PlayerEvent::Position { position_ms, .. } if *position_ms > 100),
    );

    let event_count_before = pause_and_assert_position_freezes(&player, &recorder);
    player.play().unwrap();
    recorder.wait_for_event_after(event_count_before, PlayerEvent::Resumed);

    toggle_pause_and_wait_for(&player, &recorder, PlayerEvent::Paused);
    toggle_pause_and_wait_for(&player, &recorder, PlayerEvent::Resumed);

    assert_eq!(
        recorder.milestones(),
        vec![
            PlayerEvent::Started { uid: 1 },
            PlayerEvent::Paused,
            PlayerEvent::Resumed,
            PlayerEvent::Paused,
            PlayerEvent::Resumed,
        ]
    );
}

#[test]
fn load_unpauses() {
    let (player, recorder) = player();
    player.pause().unwrap();
    recorder.wait_for_event(PlayerEvent::Paused);
    player.load(1, &sine_wave_url(1), 0).unwrap();
    recorder.wait_for_event(PlayerEvent::Idle);
    assert_eq!(
        recorder.milestones(),
        vec![
            PlayerEvent::Paused,
            PlayerEvent::Resumed,
            PlayerEvent::Started { uid: 1 },
            PlayerEvent::TrackEnded { uid: 1 },
            PlayerEvent::Idle,
        ]
    );
}

#[test]
fn seek_moves_position() {
    let (player, recorder) = player();
    player.load(1, &sine_wave_url(20), 0).unwrap();
    recorder.wait_for_event(PlayerEvent::Started { uid: 1 });
    let event_count_before = recorder.event_count();
    player.seek(12_000).unwrap();
    recorder.wait_after(event_count_before, "position after seek", |event| {
        matches!(event, PlayerEvent::Position { position_ms, .. } if (12_000..19_000).contains(position_ms))
    });
    assert!(player.position_ms().unwrap() >= 12_000);

    player.pause().unwrap();
    recorder.wait_for_event(PlayerEvent::Paused);
    let event_count_before = recorder.event_count();
    player.seek(3_000).unwrap();
    recorder.wait_after(event_count_before, "position after paused seek", |event| {
        matches!(event, PlayerEvent::Position { position_ms, .. } if (3_000..3_100).contains(position_ms))
    });
    assert!(!has_error(&recorder.snapshot()));
}

#[test]
fn gapless_transition_to_preloaded_track() {
    let (player, recorder) = player();
    player.load(1, &sine_wave_url(1), 0).unwrap();
    player.preload_next(2, &sine_wave_url(1)).unwrap();
    recorder.wait_for_event(PlayerEvent::Idle);

    assert_eq!(
        recorder.milestones(),
        vec![
            PlayerEvent::Started { uid: 1 },
            PlayerEvent::TrackEnded { uid: 1 },
            PlayerEvent::Started { uid: 2 },
            PlayerEvent::TrackEnded { uid: 2 },
            PlayerEvent::Idle,
        ]
    );
}

#[test]
fn sequential_transition_without_gapless() {
    let (player, recorder) = player_with(PlayerConfig {
        gapless: false,
        ..PlayerConfig::default()
    });
    player.load(1, &sine_wave_url(1), 0).unwrap();
    player.preload_next(2, &sine_wave_url(1)).unwrap();
    recorder.wait_for_event(PlayerEvent::Idle);

    assert_eq!(
        recorder.milestones(),
        vec![
            PlayerEvent::Started { uid: 1 },
            PlayerEvent::TrackEnded { uid: 1 },
            PlayerEvent::Started { uid: 2 },
            PlayerEvent::TrackEnded { uid: 2 },
            PlayerEvent::Idle,
        ]
    );
}

#[test]
fn preload_replaces_previous_preload() {
    let (player, recorder) = player();
    player.load(1, &sine_wave_url(1), 0).unwrap();
    player.preload_next(2, &sine_wave_url(1)).unwrap();
    player.preload_next(3, &sine_wave_url(1)).unwrap();
    recorder.wait_for_event(PlayerEvent::Idle);

    assert_eq!(
        recorder.milestones(),
        vec![
            PlayerEvent::Started { uid: 1 },
            PlayerEvent::TrackEnded { uid: 1 },
            PlayerEvent::Started { uid: 3 },
            PlayerEvent::TrackEnded { uid: 3 },
            PlayerEvent::Idle,
        ]
    );
}

#[test]
fn clear_next_removes_preloaded_track() {
    let (player, recorder) = player();
    player.load(1, &sine_wave_url(1), 0).unwrap();
    player.preload_next(2, &sine_wave_url(1)).unwrap();
    player.clear_next().unwrap();
    recorder.wait_for_event(PlayerEvent::Idle);

    assert_eq!(
        recorder.milestones(),
        vec![
            PlayerEvent::Started { uid: 1 },
            PlayerEvent::TrackEnded { uid: 1 },
            PlayerEvent::Idle,
        ]
    );
}

#[test]
fn preload_after_gapless_transition_keeps_chain_going() {
    let recorder = Arc::new(Recorder::default());
    let player_slot: Arc<OnceLock<Weak<Player>>> = Arc::new(OnceLock::new());
    let event_sink = Arc::clone(&recorder);
    let callback_player_slot = Arc::clone(&player_slot);
    let player = Arc::new(
        Player::with_null_output(
            PlayerConfig::default(),
            Box::new(move |event| {
                if let PlayerEvent::Started { uid } = event
                    && uid < 3
                    && let Some(player) = callback_player_slot.get().and_then(Weak::upgrade)
                {
                    player.preload_next(uid + 1, &sine_wave_url(1)).unwrap();
                }
                event_sink.push(event);
            }),
        )
        .unwrap(),
    );
    player_slot.set(Arc::downgrade(&player)).unwrap();

    player.load(1, &sine_wave_url(1), 0).unwrap();
    recorder.wait_for_event(PlayerEvent::Idle);

    assert_eq!(
        recorder.milestones(),
        vec![
            PlayerEvent::Started { uid: 1 },
            PlayerEvent::TrackEnded { uid: 1 },
            PlayerEvent::Started { uid: 2 },
            PlayerEvent::TrackEnded { uid: 2 },
            PlayerEvent::Started { uid: 3 },
            PlayerEvent::TrackEnded { uid: 3 },
            PlayerEvent::Idle,
        ]
    );
}

#[test]
fn stop_does_not_report_track_end() {
    let (player, recorder) = player();
    player.load(1, &sine_wave_url(10), 0).unwrap();
    player.preload_next(2, &sine_wave_url(1)).unwrap();
    recorder.wait_for_event(PlayerEvent::Started { uid: 1 });
    player.stop().unwrap();
    recorder.wait_for_event(PlayerEvent::Idle);
    thread::sleep(Duration::from_millis(300));

    assert_eq!(
        recorder.milestones(),
        vec![PlayerEvent::Started { uid: 1 }, PlayerEvent::Idle]
    );
    assert_eq!(player.position_ms(), None);
}

#[test]
fn load_replaces_current_track_without_track_end() {
    let (player, recorder) = player();
    player.load(1, &sine_wave_url(10), 0).unwrap();
    recorder.wait_for_event(PlayerEvent::Started { uid: 1 });
    player.load(2, &sine_wave_url(1), 0).unwrap();
    recorder.wait_for_event(PlayerEvent::Idle);

    assert_eq!(
        recorder.milestones(),
        vec![
            PlayerEvent::Started { uid: 1 },
            PlayerEvent::Started { uid: 2 },
            PlayerEvent::TrackEnded { uid: 2 },
            PlayerEvent::Idle,
        ]
    );
}

#[test]
fn load_failure_reports_error_with_uid() {
    let (player, recorder) = player();
    player
        .load(7, "/nonexistent/feishin-player-test.flac", 0)
        .unwrap();
    recorder.wait_for_event(PlayerEvent::Idle);

    let milestones = recorder.milestones();
    assert!(
        matches!(
            milestones.as_slice(),
            [PlayerEvent::Error { uid: Some(7), .. }, PlayerEvent::Idle]
        ),
        "{milestones:?}"
    );
}

#[test]
fn failing_preload_reports_error_and_goes_idle() {
    let (player, recorder) = player();
    player.load(1, &sine_wave_url(1), 0).unwrap();
    player
        .preload_next(2, "/nonexistent/feishin-player-test.flac")
        .unwrap();
    recorder.wait_for_event(PlayerEvent::Idle);

    let milestones = recorder.milestones();
    assert!(
        matches!(
            milestones.as_slice(),
            [
                PlayerEvent::Started { uid: 1 },
                PlayerEvent::TrackEnded { uid: 1 },
                PlayerEvent::Error { uid: Some(2), .. },
                PlayerEvent::Idle,
            ]
        ),
        "{milestones:?}"
    );
}

#[test]
fn seek_while_idle_fails() {
    let (player, _recorder) = player();
    assert!(player.seek(1000).is_err());
    assert_eq!(player.position_ms(), None);
}

#[test]
fn settings_and_devices() {
    let (player, recorder) = player_with(PlayerConfig {
        replay_gain: ReplayGainMode::Album,
        volume: 40,
        ..PlayerConfig::default()
    });
    player.set_volume(55).unwrap();
    player.set_volume(250).unwrap();
    player.set_muted(true).unwrap();
    player.set_muted(false).unwrap();
    player.set_replay_gain(ReplayGainMode::Track).unwrap();
    player.set_replay_gain(ReplayGainMode::Off).unwrap();
    player.set_audio_device(Some("auto")).unwrap();
    player.set_audio_device(None).unwrap();

    let devices = player.audio_devices().unwrap();
    assert!(
        devices.iter().any(|device| device.name == "auto"),
        "{devices:?}"
    );
    assert!(devices.iter().all(|device| !device.name.is_empty()));

    player.load(1, &sine_wave_url(1), 0).unwrap();
    recorder.wait_for_event(PlayerEvent::Idle);
    assert!(!has_error(&recorder.snapshot()));
}

#[test]
fn player_is_usable_across_threads() {
    let (player, recorder) = player();
    let player = Arc::new(player);
    let worker = {
        let player = Arc::clone(&player);
        thread::spawn(move || player.load(1, &sine_wave_url(10), 0))
    };
    worker.join().unwrap().unwrap();
    recorder.wait_for_event(PlayerEvent::Started { uid: 1 });
    let pauser = {
        let player = Arc::clone(&player);
        thread::spawn(move || player.pause())
    };
    pauser.join().unwrap().unwrap();
    recorder.wait_for_event(PlayerEvent::Paused);
    drop(player);
}
