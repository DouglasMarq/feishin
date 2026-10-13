use std::sync::{Arc, Condvar, Mutex, OnceLock, Weak};
use std::thread;
use std::time::{Duration, Instant};

use feishin_player::{Player, PlayerConfig, PlayerEvent, ReplayGainMode};

const TIMEOUT: Duration = Duration::from_secs(10);

fn sine(seconds: u32) -> String {
    format!("av://lavfi:sine=frequency=440:duration={seconds}")
}

#[derive(Default)]
struct Recorder {
    events: Mutex<Vec<PlayerEvent>>,
    changed: Condvar,
}

impl Recorder {
    fn push(&self, event: PlayerEvent) {
        self.events.lock().unwrap().push(event);
        self.changed.notify_all();
    }

    fn snapshot(&self) -> Vec<PlayerEvent> {
        self.events.lock().unwrap().clone()
    }

    fn wait_for(&self, what: &str, pred: impl Fn(&PlayerEvent) -> bool) -> PlayerEvent {
        let deadline = Instant::now() + TIMEOUT;
        let mut events = self.events.lock().unwrap();
        let mut seen = 0;
        loop {
            if let Some(found) = events[seen..].iter().find(|e| pred(e)) {
                return found.clone();
            }
            seen = events.len();
            let now = Instant::now();
            assert!(now < deadline, "timed out waiting for {what}: {events:?}");
            events = self.changed.wait_timeout(events, deadline - now).unwrap().0;
        }
    }

    fn wait_for_event(&self, expected: PlayerEvent) {
        self.wait_for(&format!("{expected:?}"), |e| *e == expected);
    }

    fn wait_after(&self, mark: usize, what: &str, pred: impl Fn(&PlayerEvent) -> bool) {
        let deadline = Instant::now() + TIMEOUT;
        let mut events = self.events.lock().unwrap();
        while !events[mark..].iter().any(&pred) {
            let now = Instant::now();
            assert!(now < deadline, "timed out waiting for {what}: {events:?}");
            events = self.changed.wait_timeout(events, deadline - now).unwrap().0;
        }
    }

    fn len(&self) -> usize {
        self.events.lock().unwrap().len()
    }

    fn milestones(&self) -> Vec<PlayerEvent> {
        self.snapshot()
            .into_iter()
            .filter(|e| !matches!(e, PlayerEvent::Position { .. }))
            .collect()
    }

    fn positions(&self) -> Vec<(u64, Option<u64>)> {
        self.snapshot()
            .into_iter()
            .filter_map(|e| match e {
                PlayerEvent::Position {
                    position_ms,
                    duration_ms,
                } => Some((position_ms, duration_ms)),
                _ => None,
            })
            .collect()
    }
}

fn player_with(config: PlayerConfig) -> (Player, Arc<Recorder>) {
    let recorder = Arc::new(Recorder::default());
    let sink = Arc::clone(&recorder);
    let player = Player::with_null_output(config, Box::new(move |e| sink.push(e))).unwrap();
    (player, recorder)
}

fn player() -> (Player, Arc<Recorder>) {
    player_with(PlayerConfig::default())
}

fn has_error(events: &[PlayerEvent]) -> bool {
    events
        .iter()
        .any(|e| matches!(e, PlayerEvent::Error { .. }))
}

#[test]
fn load_plays_to_natural_end() {
    let (player, rec) = player();
    player.load(1, &sine(1), 0).unwrap();
    rec.wait_for_event(PlayerEvent::Idle);

    assert_eq!(
        rec.milestones(),
        vec![
            PlayerEvent::Started { uid: 1 },
            PlayerEvent::TrackEnded { uid: 1 },
            PlayerEvent::Idle,
        ]
    );
    let positions = rec.positions();
    assert!(positions.len() >= 3, "{positions:?}");
    assert!(
        positions.windows(2).all(|w| w[0].0 <= w[1].0),
        "{positions:?}"
    );
    let duration = positions.iter().find_map(|p| p.1).unwrap();
    assert!((900..=1100).contains(&duration), "{duration}");
}

#[test]
fn position_events_arrive_about_every_250ms() {
    let (player, rec) = player();
    player.load(1, &sine(3), 0).unwrap();
    rec.wait_for_event(PlayerEvent::Started { uid: 1 });
    let start = rec.len();
    thread::sleep(Duration::from_millis(1600));
    let count = rec.snapshot()[start..]
        .iter()
        .filter(|e| matches!(e, PlayerEvent::Position { .. }))
        .count();
    assert!((4..=9).contains(&count), "{count} positions in 1.6 s");
    assert!(player.position_ms().is_some());
}

#[test]
fn load_starts_at_offset() {
    let (player, rec) = player();
    player.load(1, &sine(10), 4000).unwrap();
    rec.wait_for_event(PlayerEvent::Started { uid: 1 });
    rec.wait_for("position", |e| matches!(e, PlayerEvent::Position { .. }));
    assert!(
        rec.positions().iter().all(|p| p.0 >= 4000),
        "{:?}",
        rec.positions()
    );
}

#[test]
fn pause_and_resume() {
    let (player, rec) = player();
    player.load(1, &sine(10), 0).unwrap();
    rec.wait_for(
        "playback underway",
        |e| matches!(e, PlayerEvent::Position { position_ms, .. } if *position_ms > 100),
    );

    player.pause().unwrap();
    rec.wait_for_event(PlayerEvent::Paused);
    thread::sleep(Duration::from_millis(200));
    let mark = rec.len();
    let paused_at = player.position_ms().unwrap();
    thread::sleep(Duration::from_millis(700));
    let after_pause: Vec<_> = rec.snapshot()[mark..].to_vec();
    assert!(
        !after_pause
            .iter()
            .any(|e| matches!(e, PlayerEvent::Position { .. })),
        "{after_pause:?}"
    );
    assert_eq!(player.position_ms(), Some(paused_at));

    player.play().unwrap();
    rec.wait_after(mark, "Resumed", |e| *e == PlayerEvent::Resumed);

    let mark = rec.len();
    player.toggle_pause().unwrap();
    rec.wait_after(mark, "Paused", |e| *e == PlayerEvent::Paused);
    let mark = rec.len();
    player.toggle_pause().unwrap();
    rec.wait_after(mark, "Resumed", |e| *e == PlayerEvent::Resumed);

    assert_eq!(
        rec.milestones(),
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
    let (player, rec) = player();
    player.pause().unwrap();
    rec.wait_for_event(PlayerEvent::Paused);
    player.load(1, &sine(1), 0).unwrap();
    rec.wait_for_event(PlayerEvent::Idle);
    assert_eq!(
        rec.milestones(),
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
    let (player, rec) = player();
    player.load(1, &sine(20), 0).unwrap();
    rec.wait_for_event(PlayerEvent::Started { uid: 1 });
    let mark = rec.len();
    player.seek(12_000).unwrap();
    rec.wait_after(mark, "position after seek", |e| {
        matches!(e, PlayerEvent::Position { position_ms, .. } if (12_000..13_000).contains(position_ms))
    });
    assert!(player.position_ms().unwrap() >= 12_000);

    player.pause().unwrap();
    rec.wait_for_event(PlayerEvent::Paused);
    let mark = rec.len();
    player.seek(3_000).unwrap();
    rec.wait_after(mark, "position after paused seek", |e| {
        matches!(e, PlayerEvent::Position { position_ms, .. } if (3_000..3_100).contains(position_ms))
    });
    assert!(!has_error(&rec.snapshot()));
}

#[test]
fn gapless_transition_to_preloaded_track() {
    let (player, rec) = player();
    player.load(1, &sine(1), 0).unwrap();
    player.preload_next(2, &sine(1)).unwrap();
    rec.wait_for_event(PlayerEvent::Idle);

    assert_eq!(
        rec.milestones(),
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
    let (player, rec) = player_with(PlayerConfig {
        gapless: false,
        ..PlayerConfig::default()
    });
    player.load(1, &sine(1), 0).unwrap();
    player.preload_next(2, &sine(1)).unwrap();
    rec.wait_for_event(PlayerEvent::Idle);

    assert_eq!(
        rec.milestones(),
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
    let (player, rec) = player();
    player.load(1, &sine(1), 0).unwrap();
    player.preload_next(2, &sine(1)).unwrap();
    player.preload_next(3, &sine(1)).unwrap();
    rec.wait_for_event(PlayerEvent::Idle);

    assert_eq!(
        rec.milestones(),
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
    let (player, rec) = player();
    player.load(1, &sine(1), 0).unwrap();
    player.preload_next(2, &sine(1)).unwrap();
    player.clear_next().unwrap();
    rec.wait_for_event(PlayerEvent::Idle);

    assert_eq!(
        rec.milestones(),
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
    let slot: Arc<OnceLock<Weak<Player>>> = Arc::new(OnceLock::new());
    let sink = Arc::clone(&recorder);
    let player_ref = Arc::clone(&slot);
    let player = Arc::new(
        Player::with_null_output(
            PlayerConfig::default(),
            Box::new(move |event| {
                if let PlayerEvent::Started { uid } = event
                    && uid < 3
                    && let Some(player) = player_ref.get().and_then(Weak::upgrade)
                {
                    player.preload_next(uid + 1, &sine(1)).unwrap();
                }
                sink.push(event);
            }),
        )
        .unwrap(),
    );
    slot.set(Arc::downgrade(&player)).unwrap();

    player.load(1, &sine(1), 0).unwrap();
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
    let (player, rec) = player();
    player.load(1, &sine(10), 0).unwrap();
    player.preload_next(2, &sine(1)).unwrap();
    rec.wait_for_event(PlayerEvent::Started { uid: 1 });
    player.stop().unwrap();
    rec.wait_for_event(PlayerEvent::Idle);
    thread::sleep(Duration::from_millis(300));

    assert_eq!(
        rec.milestones(),
        vec![PlayerEvent::Started { uid: 1 }, PlayerEvent::Idle]
    );
    assert_eq!(player.position_ms(), None);
}

#[test]
fn load_replaces_current_track_without_track_end() {
    let (player, rec) = player();
    player.load(1, &sine(10), 0).unwrap();
    rec.wait_for_event(PlayerEvent::Started { uid: 1 });
    player.load(2, &sine(1), 0).unwrap();
    rec.wait_for_event(PlayerEvent::Idle);

    assert_eq!(
        rec.milestones(),
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
    let (player, rec) = player();
    player
        .load(7, "/nonexistent/feishin-player-test.flac", 0)
        .unwrap();
    rec.wait_for_event(PlayerEvent::Idle);

    let milestones = rec.milestones();
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
    let (player, rec) = player();
    player.load(1, &sine(1), 0).unwrap();
    player
        .preload_next(2, "/nonexistent/feishin-player-test.flac")
        .unwrap();
    rec.wait_for_event(PlayerEvent::Idle);

    let milestones = rec.milestones();
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
    let (player, _rec) = player();
    assert!(player.seek(1000).is_err());
    assert_eq!(player.position_ms(), None);
}

#[test]
fn settings_and_devices() {
    let (player, rec) = player_with(PlayerConfig {
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
    assert!(devices.iter().any(|d| d.name == "auto"), "{devices:?}");
    assert!(devices.iter().all(|d| !d.name.is_empty()));

    player.load(1, &sine(1), 0).unwrap();
    rec.wait_for_event(PlayerEvent::Idle);
    assert!(!has_error(&rec.snapshot()));
}

#[test]
fn player_is_usable_across_threads() {
    let (player, rec) = player();
    let player = Arc::new(player);
    let worker = {
        let player = Arc::clone(&player);
        thread::spawn(move || player.load(1, &sine(10), 0))
    };
    worker.join().unwrap().unwrap();
    rec.wait_for_event(PlayerEvent::Started { uid: 1 });
    let pauser = {
        let player = Arc::clone(&player);
        thread::spawn(move || player.pause())
    };
    pauser.join().unwrap().unwrap();
    rec.wait_for_event(PlayerEvent::Paused);
    drop(player);
}
