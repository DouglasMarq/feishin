use feishin_core::model::{Paging, SongListQuery, SongSort};
use feishin_core::queue::{RepeatMode, Uid};
use feishin_player::PlayerEvent;

use super::harness::{AudioCall, start_harness, start_harness_with, test_settings, test_songs};
use crate::controller::Command;

#[tokio::test]
async fn play_from_list_queues_the_whole_list_and_loads_the_clicked_song() {
    let mut harness = start_harness_with(test_settings(false, false), 1234, None);
    let query = SongListQuery {
        sort: SongSort::Name,
        ..SongListQuery::default()
    };
    let paging = Paging {
        start_index: 0,
        limit: 2000,
    };
    let expected: Vec<String> =
        feishin_api::MusicServer::song_list(harness.server.as_ref(), &query, paging)
            .await
            .map(|page| page.items.into_iter().map(|song| song.id).collect())
            .unwrap_or_default();
    let clicked = expected[700].clone();

    harness.play_from_list(query.clone(), &clicked).await;

    assert_eq!(harness.queued_ids(), expected);
    assert_eq!(harness.current_id(), Some(clicked.clone()));
    let uid = harness.current_uid().unwrap_or(Uid::MAX);
    assert_eq!(harness.audio.take(), [AudioCall::Load(uid, 0)]);

    harness.start_current();
    harness.player(PlayerEvent::Position {
        position_ms: 30_000,
        duration_ms: None,
    });
    harness.play_from_list(query, &clicked).await;
    let restarted = harness.current_uid().unwrap_or(Uid::MAX);
    assert_eq!(harness.current_id(), Some(clicked));
    assert_eq!(harness.audio.take(), [AudioCall::Load(restarted, 0)]);
}

#[tokio::test]
async fn stale_list_fetches_are_ignored() {
    let mut harness = start_harness(test_settings(false, false));
    let query = SongListQuery::default();
    harness.controller.handle(Command::PlayFromList {
        query: query.clone(),
        song_id: "song-1".into(),
    });
    harness.controller.handle(Command::PlayFromList {
        query,
        song_id: "song-2".into(),
    });
    harness.handle_follow_ups(2).await;
    assert_eq!(harness.current_id().as_deref(), Some("song-2"));
    assert_eq!(harness.audio.take().len(), 1);
}

#[tokio::test]
async fn track_end_advances_and_stops_after_the_last_song() {
    let mut harness = start_harness(test_settings(false, false));
    harness.controller.play_now(test_songs(2, 1000), "s0");
    let first = harness.start_current();
    harness.audio.take();

    harness.player(PlayerEvent::TrackEnded { uid: first });
    let second = harness.uid_of("s1").unwrap_or(Uid::MAX);
    assert_eq!(harness.audio.take(), [AudioCall::Load(second, 0)]);

    harness.start_current();
    harness.player(PlayerEvent::TrackEnded { uid: second });
    assert_eq!(harness.audio.take(), [AudioCall::Stop]);
    assert!(!harness.controller.is_playing());
}

#[tokio::test]
async fn gapless_preloads_on_start_and_follows_the_takeover() {
    let mut harness = start_harness(test_settings(true, false));
    harness.controller.play_now(test_songs(3, 1000), "s0");
    let first = harness.start_current();
    let second = harness.uid_of("s1").unwrap_or(Uid::MAX);
    let third = harness.uid_of("s2").unwrap_or(Uid::MAX);
    assert_eq!(
        harness.audio.take(),
        [AudioCall::Load(first, 0), AudioCall::Preload(second)]
    );

    harness.player(PlayerEvent::TrackEnded { uid: first });
    assert_eq!(harness.audio.take(), []);
    harness.player(PlayerEvent::Started { uid: second });
    assert_eq!(harness.current_uid(), Some(second));
    assert_eq!(harness.audio.take(), [AudioCall::Preload(third)]);

    harness.controller.handle(Command::Remove(vec![third]));
    assert_eq!(harness.audio.take(), [AudioCall::ClearNext]);
}

#[tokio::test]
async fn repeat_one_preloads_the_same_song() {
    let mut harness = start_harness(test_settings(true, false));
    harness.controller.play_now(test_songs(2, 1000), "s0");
    harness.controller.handle(Command::CycleRepeat);
    harness.controller.handle(Command::CycleRepeat);
    assert_eq!(harness.controller.queue().repeat(), RepeatMode::One);
    let first = harness.start_current();
    assert_eq!(
        harness.audio.take().last(),
        Some(&AudioCall::Preload(first))
    );
    harness.player(PlayerEvent::Started { uid: first });
    assert_eq!(harness.current_uid(), Some(first));
}

#[tokio::test]
async fn previous_restarts_after_ten_seconds() {
    let mut harness = start_harness(test_settings(false, false));
    harness.controller.play_now(test_songs(2, 100_000), "s1");
    harness.start_current();
    harness.player(PlayerEvent::Position {
        position_ms: 12_000,
        duration_ms: None,
    });
    harness.audio.take();
    harness.controller.handle(Command::Previous);
    assert_eq!(harness.audio.take(), [AudioCall::Seek(0)]);
    harness.controller.handle(Command::Previous);
    let first = harness.uid_of("s0").unwrap_or(Uid::MAX);
    assert_eq!(harness.audio.take(), [AudioCall::Load(first, 0)]);
}

#[tokio::test]
async fn previous_goes_back_early_in_a_track_and_restarts_the_first_one() {
    let mut harness = start_harness(test_settings(false, false));
    harness.controller.play_now(test_songs(2, 100_000), "s1");
    harness.start_current();
    harness.player(PlayerEvent::Position {
        position_ms: 9_000,
        duration_ms: None,
    });
    harness.audio.take();
    harness.controller.handle(Command::Previous);
    let first = harness.uid_of("s0").unwrap_or(Uid::MAX);
    assert_eq!(harness.audio.take(), [AudioCall::Load(first, 0)]);

    harness.start_current();
    harness.controller.handle(Command::Previous);
    assert_eq!(harness.audio.take(), [AudioCall::Seek(0)]);
}
