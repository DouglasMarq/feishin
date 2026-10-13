use feishin_api::MusicServer;
use feishin_core::model::SongListQuery;
use feishin_core::queue::Uid;
use feishin_player::PlayerEvent;

use super::harness::{AudioCall, settle, song_ids, start_harness, test_settings, test_songs};
use crate::controller::Command;

#[tokio::test]
async fn queue_changes_are_saved_to_the_server_once_debounced() {
    let mut harness = start_harness(test_settings(false, false));
    harness.controller.play_now(test_songs(4, 100_000), "s1");
    let first = harness.start_current();
    harness.controller.handle(Command::Remove(vec![first]));
    harness.controller.handle(Command::TogglePause);
    settle().await;

    let (saves, saved) = harness.server.saved_play_queue();
    assert_eq!(saves, 1);
    let saved = saved.unwrap_or_else(|| panic!("queue saved"));
    assert_eq!(song_ids(&saved.songs), ["s0", "s2", "s3"]);
    assert_eq!(saved.current_song_id.as_deref(), Some("s2"));

    harness.controller.handle(Command::Seek(42_000));
    settle().await;
    let (saves, saved) = harness.server.saved_play_queue();
    assert_eq!(saves, 2);
    assert_eq!(saved.map(|queue| queue.position_ms), Some(42_000));

    harness.controller.handle(Command::Seek(42_000));
    settle().await;
    assert_eq!(harness.server.saved_play_queue().0, 2);
}

#[tokio::test]
async fn shutdown_saves_the_queue_with_the_position() {
    let mut harness = start_harness(test_settings(false, false));
    harness.controller.play_now(test_songs(3, 100_000), "s2");
    harness.start_current();
    harness.player(PlayerEvent::Position {
        position_ms: 12_345,
        duration_ms: None,
    });
    assert!(harness.shutdown().await.is_ok());
    let saved = harness.server.saved_play_queue().1;
    assert_eq!(
        saved.map(|queue| (queue.current_song_id, queue.position_ms)),
        Some((Some("s2".to_owned()), 12_345))
    );
}

#[tokio::test]
async fn empty_queue_is_restored_from_the_server_without_playing() {
    let mut harness = start_harness(test_settings(false, false));
    let saved: Vec<String> = ["song-3", "song-4", "song-5"].map(String::from).into();
    let seeded = harness
        .server
        .save_play_queue(&saved, Some("song-4"), 5_000)
        .await;
    assert!(seeded.is_ok());

    harness.connect();
    let command = harness.commands.recv().await;
    assert!(matches!(command, Some(Command::RestoreQueue { .. })));
    if let Some(command) = command {
        harness.controller.handle(command);
    }
    assert_eq!(harness.queued_ids(), saved);
    assert_eq!(harness.current_id().as_deref(), Some("song-4"));
    assert!(!harness.controller.is_playing());
    assert!(harness.audio.take().is_empty());

    harness.controller.handle(Command::TogglePause);
    let uid = harness.current_uid().unwrap_or(Uid::MAX);
    assert_eq!(harness.audio.take(), [AudioCall::Load(uid, 5_000)]);
}

#[tokio::test]
async fn server_queue_does_not_replace_a_local_queue_or_a_newer_play() {
    let mut harness = start_harness(test_settings(false, false));
    let seeded = harness
        .server
        .save_play_queue(&["song-1".to_owned()], None, 0)
        .await;
    assert!(seeded.is_ok());

    harness.connect();
    harness.request_from_list(SongListQuery::default(), 0, "song-2");
    harness.finish_streams().await;
    assert_eq!(harness.current_id().as_deref(), Some("song-2"));
    assert!(harness.controller.queue().len() > 1);

    harness.connect();
    settle().await;
    assert!(harness.commands.try_recv().is_err());
}
