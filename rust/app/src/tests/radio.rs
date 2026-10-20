use feishin_core::model::RadioStation;
use feishin_core::queue::Uid;
use feishin_platform::MediaControlEvent;
use feishin_player::PlayerEvent;

use super::harness::{AudioCall, Harness, start_harness, test_settings, test_songs};
use crate::controller::{Command, RADIO_STREAM_UID, RadioCommand, UiUpdate};
use crate::store::Store;

fn station() -> RadioStation {
    RadioStation {
        id: "7".into(),
        name: "Jazz FM".into(),
        stream_url: "https://jazz.example.com/stream".into(),
        homepage_url: None,
    }
}

fn radio(harness: &mut Harness, command: RadioCommand) {
    harness.controller.handle(Command::Radio(command));
}

fn queue_at_thirty_seconds(is_gapless: bool) -> (Harness, Uid) {
    let mut harness = start_harness(test_settings(is_gapless, false));
    harness.controller.play_now(test_songs(3, 100_000), "s1");
    let current = harness.start_current();
    harness.player(PlayerEvent::Position {
        position_ms: 30_000,
        duration_ms: Some(100_000),
    });
    harness.audio.take();
    (harness, current)
}

fn tune_in(harness: &mut Harness) {
    radio(harness, RadioCommand::Play(station()));
    harness.player(PlayerEvent::Started {
        uid: RADIO_STREAM_UID,
    });
}

fn shows_station(harness: &Harness, stream_title: Option<&str>) -> bool {
    harness.ui.any(|update| {
        matches!(update, UiUpdate::Radio(Some(now))
            if now.station_name == "Jazz FM" && now.stream_title.as_deref() == stream_title)
    })
}

#[tokio::test]
async fn a_station_replaces_queue_playback_without_touching_the_queue() {
    let (mut harness, current) = queue_at_thirty_seconds(true);
    let queued = harness.queued_ids();

    radio(&mut harness, RadioCommand::Play(station()));

    assert_eq!(
        harness.audio.take(),
        [AudioCall::ClearNext, AudioCall::Load(RADIO_STREAM_UID, 0)]
    );
    assert_eq!(harness.queued_ids(), queued);
    assert_eq!(harness.current_uid(), Some(current));
    assert!(shows_station(&harness, None));

    harness.player(PlayerEvent::Started {
        uid: RADIO_STREAM_UID,
    });
    harness.player(PlayerEvent::StreamTitle(Some("Artist - Song".into())));
    assert!(harness.controller.is_playing());
    assert!(shows_station(&harness, Some("Artist - Song")));
    assert_eq!(harness.audio.take(), []);
}

#[tokio::test]
async fn skipping_and_seeking_do_nothing_while_a_station_plays() {
    let (mut harness, current) = queue_at_thirty_seconds(false);
    tune_in(&mut harness);
    harness.audio.take();

    for command in [
        Command::Next,
        Command::Previous,
        Command::Seek(5_000),
        Command::Media(MediaControlEvent::Next),
        Command::Media(MediaControlEvent::SeekBy { offset_ms: 10_000 }),
    ] {
        harness.controller.handle(command);
    }

    assert_eq!(harness.audio.take(), []);
    assert_eq!(harness.current_uid(), Some(current));
}

#[tokio::test]
async fn playing_a_queue_track_leaves_radio_mode() {
    let (mut harness, current) = queue_at_thirty_seconds(false);
    tune_in(&mut harness);
    harness.audio.take();
    harness.ui.clear();

    harness.controller.handle(Command::PlayUid(current));

    assert_eq!(harness.audio.take(), [AudioCall::Load(current, 0)]);
    assert!(
        harness
            .ui
            .any(|update| matches!(update, UiUpdate::Radio(None)))
    );
    harness.player(PlayerEvent::StreamTitle(Some("Late title".into())));
    assert!(!shows_station(&harness, Some("Late title")));
}

#[tokio::test]
async fn stopping_the_station_resumes_the_queue_track_where_it_was() {
    let (mut harness, current) = queue_at_thirty_seconds(false);
    tune_in(&mut harness);
    harness.player(PlayerEvent::Position {
        position_ms: 90_000,
        duration_ms: None,
    });
    assert!(harness.ui.any(|update| matches!(
        update,
        UiUpdate::Playback {
            position_ms: 90_000,
            duration_ms: 0,
            ..
        }
    )));
    harness.audio.take();

    radio(&mut harness, RadioCommand::Stop);
    assert_eq!(harness.audio.take(), [AudioCall::Stop]);
    assert!(harness.ui.any(|update| matches!(
        update,
        UiUpdate::Playback {
            is_playing: false,
            position_ms: 30_000,
            duration_ms: 100_000
        }
    )));

    harness.controller.handle(Command::TogglePause);
    assert_eq!(harness.audio.take(), [AudioCall::Load(current, 30_000)]);
}

#[tokio::test]
async fn a_station_that_stops_streaming_is_reported_and_can_be_restarted() {
    let (mut harness, _current) = queue_at_thirty_seconds(false);
    tune_in(&mut harness);
    harness.audio.take();

    harness.player(PlayerEvent::TrackEnded {
        uid: RADIO_STREAM_UID,
    });
    assert!(!harness.controller.is_playing());
    assert_eq!(harness.audio.take(), []);
    assert!(
        harness
            .ui
            .errors()
            .iter()
            .any(|error| error.contains("stopped streaming"))
    );

    harness.controller.handle(Command::TogglePause);
    assert_eq!(harness.audio.take(), [AudioCall::Load(RADIO_STREAM_UID, 0)]);
}

#[tokio::test]
async fn a_failing_station_shows_why() {
    let (mut harness, _current) = queue_at_thirty_seconds(false);
    tune_in(&mut harness);

    harness.player(PlayerEvent::Error {
        uid: Some(RADIO_STREAM_UID),
        message: "HTTP 404".into(),
    });

    assert!(!harness.controller.is_playing());
    assert!(
        harness
            .ui
            .errors()
            .contains(&"Playing Jazz FM failed: HTTP 404".to_owned())
    );
}

#[tokio::test]
async fn removing_queue_songs_keeps_the_station_playing() {
    let (mut harness, current) = queue_at_thirty_seconds(false);
    tune_in(&mut harness);
    harness.audio.take();

    let everything: Vec<Uid> = ["s0", "s1", "s2"]
        .iter()
        .filter_map(|song_id| harness.uid_of(song_id))
        .collect();
    assert!(everything.contains(&current));
    harness.controller.handle(Command::Remove(everything));

    assert_eq!(harness.audio.take(), []);
    assert!(harness.controller.is_playing());
}

#[tokio::test]
async fn favorites_are_kept_in_the_settings() {
    let mut harness = start_harness(test_settings(false, false));
    radio(&mut harness, RadioCommand::AddFavorite(station()));
    let renamed = RadioStation {
        name: "Jazz FM Renamed".into(),
        ..station()
    };
    radio(&mut harness, RadioCommand::AddFavorite(renamed.clone()));

    let saved = harness.store.load_settings().unwrap_or_default();
    assert_eq!(saved.radio.favorites, [renamed]);

    radio(&mut harness, RadioCommand::RemoveFavorite("7".into()));
    let saved = harness.store.load_settings().unwrap_or_default();
    assert!(saved.radio.favorites.is_empty());
}

#[tokio::test]
async fn stations_that_are_not_internet_streams_are_refused() {
    let (mut harness, current) = queue_at_thirty_seconds(false);
    let local_file = RadioStation {
        stream_url: "file:///home/user/secret.flac".into(),
        ..station()
    };

    radio(&mut harness, RadioCommand::Play(local_file));

    assert_eq!(harness.audio.take(), []);
    assert!(harness.controller.is_playing());
    assert_eq!(harness.current_uid(), Some(current));
    assert!(
        harness
            .ui
            .errors()
            .iter()
            .any(|error| error.contains("not an internet stream"))
    );
    assert!(
        !harness
            .ui
            .any(|update| matches!(update, UiUpdate::Radio(Some(_))))
    );
}

#[tokio::test]
async fn the_media_session_stop_button_stops_the_station() {
    let (mut harness, current) = queue_at_thirty_seconds(false);
    tune_in(&mut harness);
    harness.audio.take();
    harness.ui.clear();

    harness
        .controller
        .handle(Command::Media(MediaControlEvent::Stop));

    assert_eq!(harness.audio.take(), [AudioCall::Stop]);
    assert!(
        harness
            .ui
            .any(|update| matches!(update, UiUpdate::Radio(None)))
    );
    harness.controller.handle(Command::TogglePause);
    assert_eq!(harness.audio.take(), [AudioCall::Load(current, 30_000)]);
}

#[tokio::test]
async fn pausing_a_station_closes_the_stream_and_play_returns_to_the_live_point() {
    let (mut harness, _current) = queue_at_thirty_seconds(false);
    tune_in(&mut harness);
    harness.audio.take();

    harness.controller.handle(Command::TogglePause);
    assert_eq!(harness.audio.take(), [AudioCall::Stop]);
    assert!(!harness.controller.is_playing());
    assert!(shows_station(&harness, None));

    harness.controller.handle(Command::TogglePause);
    assert_eq!(harness.audio.take(), [AudioCall::Load(RADIO_STREAM_UID, 0)]);
}

#[tokio::test]
async fn a_queue_track_that_cannot_load_keeps_the_station_playing() {
    let (mut harness, current) = queue_at_thirty_seconds(false);
    tune_in(&mut harness);
    harness.audio.take();
    harness.ui.clear();
    harness.session.set(None);

    harness.controller.handle(Command::PlayUid(current));

    assert_eq!(harness.audio.take(), []);
    assert!(harness.controller.is_playing());
    assert!(
        !harness
            .ui
            .any(|update| matches!(update, UiUpdate::Radio(None)))
    );
    harness.player(PlayerEvent::StreamTitle(Some("Still live".into())));
    assert!(shows_station(&harness, Some("Still live")));
}
