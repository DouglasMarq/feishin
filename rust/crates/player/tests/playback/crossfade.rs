use std::sync::{Arc, OnceLock, Weak};
use std::thread;
use std::time::Duration;

use feishin_player::{MediaSource, Player, PlayerConfig, PlayerEvent, ReplayGainMode};

use crate::support::{Recorder, has_error, player_with, position_of, sine_wave_url};

const MISSING_FILE: &str = "/nonexistent/feishin-player-test.flac";
const POSITION_AFTER_SEEK_PLAYED_MS: u64 = 7_500;

fn crossfade_player(crossfade: Duration) -> (Player, Arc<Recorder>) {
    player_with(PlayerConfig {
        crossfade,
        ..PlayerConfig::default()
    })
}

fn source(uid: u64, url: &str, seconds: u32) -> MediaSource<'_> {
    MediaSource::new(uid, url).with_duration_ms(u64::from(seconds) * 1_000)
}

fn load_track(player: &Player, uid: u64, seconds: u32) {
    let url = sine_wave_url(seconds);
    player.load_source(source(uid, &url, seconds), 0).unwrap();
}

fn preload_track(player: &Player, uid: u64, seconds: u32) {
    let url = sine_wave_url(seconds);
    player.preload_source(source(uid, &url, seconds)).unwrap();
}

fn index_of(events: &[PlayerEvent], expected: &PlayerEvent) -> usize {
    events.iter().position(|event| event == expected).unwrap()
}

fn started(uid: u64) -> PlayerEvent {
    PlayerEvent::Started { uid }
}

fn ended(uid: u64) -> PlayerEvent {
    PlayerEvent::TrackEnded { uid }
}

#[test]
fn next_track_starts_before_the_current_one_ends() {
    let (player, recorder) = crossfade_player(Duration::from_secs(1));
    load_track(&player, 1, 3);
    preload_track(&player, 2, 2);
    recorder.wait_for_event(PlayerEvent::Idle);

    assert_eq!(
        recorder.milestones(),
        vec![started(1), started(2), ended(2), PlayerEvent::Idle]
    );
    let events = recorder.snapshot();
    let handover = index_of(&events, &started(2));
    let last_outgoing_position = events[..handover].iter().filter_map(position_of).max();
    assert!(
        last_outgoing_position.is_some_and(|position| (1_500..2_600).contains(&position)),
        "{events:?}"
    );
    let first_incoming_position = events[handover..].iter().find_map(position_of);
    assert!(
        first_incoming_position.is_some_and(|position| position < 1_000),
        "{events:?}"
    );
    assert!(!player.is_crossfading());
}

#[test]
fn seeking_during_a_fade_cancels_it() {
    let (player, recorder) = crossfade_player(Duration::from_secs(2));
    load_track(&player, 1, 3);
    preload_track(&player, 2, 10);
    recorder.wait_for_event(started(2));
    assert!(player.is_crossfading());

    let event_count_before = recorder.event_count();
    player.seek(6_000).unwrap();
    assert!(!player.is_crossfading());
    recorder.wait_after(event_count_before, "position after seek", |event| {
        position_of(event).is_some_and(|position| (6_000..9_000).contains(&position))
    });
    recorder.wait_after(event_count_before, "1.5 s played after the seek", |event| {
        position_of(event).is_some_and(|position| position >= POSITION_AFTER_SEEK_PLAYED_MS)
    });

    assert_eq!(recorder.milestones(), vec![started(1), started(2)]);
    assert!(player.position_ms().unwrap() >= 6_000);
}

#[test]
fn skipping_during_a_fade_cancels_it() {
    let (player, recorder) = crossfade_player(Duration::from_secs(2));
    load_track(&player, 1, 3);
    preload_track(&player, 2, 10);
    recorder.wait_for_event(started(2));
    load_track(&player, 3, 1);
    assert!(!player.is_crossfading());
    recorder.wait_for_event(PlayerEvent::Idle);

    assert_eq!(
        recorder.milestones(),
        vec![
            started(1),
            started(2),
            started(3),
            ended(3),
            PlayerEvent::Idle
        ]
    );
}

#[test]
fn pausing_during_a_fade_freezes_it() {
    let (player, recorder) = crossfade_player(Duration::from_secs(2));
    load_track(&player, 1, 4);
    preload_track(&player, 2, 3);
    recorder.wait_for_event(started(2));
    player.pause().unwrap();
    recorder.wait_for_event(PlayerEvent::Paused);
    thread::sleep(Duration::from_millis(2_500));
    assert!(player.is_crossfading());

    player.toggle_pause().unwrap();
    recorder.wait_for_event(PlayerEvent::Idle);
    assert_eq!(
        recorder.milestones(),
        vec![
            started(1),
            started(2),
            PlayerEvent::Paused,
            PlayerEvent::Resumed,
            ended(2),
            PlayerEvent::Idle
        ]
    );
}

#[test]
fn incoming_track_shorter_than_the_fade_ends_it() {
    let (player, recorder) = crossfade_player(Duration::from_secs(2));
    load_track(&player, 1, 4);
    preload_track(&player, 2, 1);
    recorder.wait_for_event(PlayerEvent::Idle);

    assert_eq!(
        recorder.milestones(),
        vec![started(1), started(2), ended(2), PlayerEvent::Idle]
    );
    assert!(!player.is_crossfading());
}

#[test]
fn failing_preload_reports_error_and_goes_idle() {
    let (player, recorder) = crossfade_player(Duration::from_secs(1));
    load_track(&player, 1, 1);
    player.preload_next(2, MISSING_FILE).unwrap();
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
fn repeating_one_song_fades_into_itself() {
    let (player, recorder) = crossfade_player(Duration::from_millis(500));
    load_track(&player, 1, 2);
    preload_track(&player, 1, 2);
    recorder.wait_for_event(PlayerEvent::Idle);

    assert_eq!(
        recorder.milestones(),
        vec![started(1), started(1), ended(1), PlayerEvent::Idle]
    );
}

#[test]
fn preloading_from_the_callback_keeps_the_chain_going() {
    let recorder = Arc::new(Recorder::default());
    let player_slot: Arc<OnceLock<Weak<Player>>> = Arc::new(OnceLock::new());
    let event_sink = Arc::clone(&recorder);
    let callback_player_slot = Arc::clone(&player_slot);
    let player = Arc::new(
        Player::with_null_output(
            PlayerConfig {
                crossfade: Duration::from_millis(500),
                ..PlayerConfig::default()
            },
            Box::new(move |event| {
                if let PlayerEvent::Started { uid } = event
                    && uid < 3
                    && let Some(player) = callback_player_slot.get().and_then(Weak::upgrade)
                {
                    preload_track(&player, uid + 1, 2);
                }
                event_sink.push(event);
            }),
        )
        .unwrap(),
    );
    player_slot.set(Arc::downgrade(&player)).unwrap();

    load_track(&player, 1, 2);
    recorder.wait_for_event(PlayerEvent::Idle);

    assert_eq!(
        recorder.milestones(),
        vec![
            started(1),
            started(2),
            started(3),
            ended(3),
            PlayerEvent::Idle
        ]
    );
}

#[test]
fn turning_crossfade_on_while_playing_fades_into_the_next_preload() {
    let (player, recorder) = player_with(PlayerConfig::default());
    load_track(&player, 1, 3);
    preload_track(&player, 2, 1);
    player.set_crossfade(Duration::from_secs(1)).unwrap();
    preload_track(&player, 2, 1);
    recorder.wait_for_event(PlayerEvent::Idle);

    assert_eq!(
        recorder.milestones(),
        vec![started(1), started(2), ended(2), PlayerEvent::Idle]
    );
}

#[test]
fn turning_crossfade_off_restores_gapless_playback() {
    let (player, recorder) = crossfade_player(Duration::from_secs(1));
    load_track(&player, 1, 1);
    preload_track(&player, 2, 1);
    player.set_crossfade(Duration::ZERO).unwrap();
    preload_track(&player, 2, 1);
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
fn output_settings_apply_to_both_decks() {
    let (player, recorder) = crossfade_player(Duration::from_secs(1));
    player.set_volume(40).unwrap();
    player.set_muted(true).unwrap();
    player.set_replay_gain(ReplayGainMode::Album).unwrap();
    player.set_audio_device(Some("auto")).unwrap();
    load_track(&player, 1, 2);
    preload_track(&player, 2, 1);
    recorder.wait_for_event(PlayerEvent::Idle);

    assert_eq!(
        recorder.milestones(),
        vec![started(1), started(2), ended(2), PlayerEvent::Idle]
    );
    assert!(!has_error(&recorder.snapshot()));
}

#[test]
fn stop_during_a_fade_silences_both_decks() {
    let (player, recorder) = crossfade_player(Duration::from_secs(2));
    load_track(&player, 1, 3);
    preload_track(&player, 2, 10);
    recorder.wait_for_event(started(2));
    player.stop().unwrap();
    recorder.wait_for_event(PlayerEvent::Idle);
    thread::sleep(Duration::from_millis(1_500));

    assert_eq!(
        recorder.milestones(),
        vec![started(1), started(2), PlayerEvent::Idle]
    );
    assert!(!player.is_crossfading());
    assert_eq!(player.position_ms(), None);
}
