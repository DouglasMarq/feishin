use feishin_core::queue::QueueTrack;

use super::harness::{AudioCall, test_songs};
use super::youtube_support::{
    EXPIRED_URL, FUTURE_URL, FakeYtDlp, play_tracks, uid_of_video, youtube_track,
};
use crate::controller::Command;

#[tokio::test]
async fn youtube_tracks_play_once_yt_dlp_resolves_them() {
    let fake = FakeYtDlp::printing(FUTURE_URL).await;
    let mut harness = fake.harness(false, false);
    play_tracks(&mut harness, vec![youtube_track("abc")]);
    assert_eq!(harness.audio.take(), []);
    assert!(!harness.controller.is_playing());

    harness.handle_next_command().await;
    let uid = uid_of_video(&harness, "abc");
    assert_eq!(harness.audio.take(), [AudioCall::Load(uid, 0)]);
    assert!(harness.controller.is_playing());
    assert_eq!(
        fake.last_arguments().last().map(String::as_str),
        Some("https://music.youtube.com/watch?v=abc")
    );
    assert!(harness.ui.errors().is_empty());
}

#[tokio::test]
async fn resolved_addresses_are_reused_until_they_expire() {
    let fake = FakeYtDlp::printing(FUTURE_URL).await;
    let mut harness = fake.harness(false, false);
    let song = QueueTrack::from(&test_songs(1, 100_000)[0]);
    play_tracks(&mut harness, vec![youtube_track("abc"), song]);
    harness.handle_next_command().await;
    harness.controller.handle(Command::Next);
    harness.controller.handle(Command::Previous);
    let uid = uid_of_video(&harness, "abc");
    assert_eq!(harness.audio.take().last(), Some(&AudioCall::Load(uid, 0)));
    assert_eq!(fake.calls(), 1);
}

#[tokio::test]
async fn expired_addresses_are_resolved_again() {
    let fake = FakeYtDlp::printing(EXPIRED_URL).await;
    let mut harness = fake.harness(false, false);
    let song = QueueTrack::from(&test_songs(1, 100_000)[0]);
    play_tracks(&mut harness, vec![youtube_track("abc"), song]);
    harness.handle_next_command().await;
    let uid = uid_of_video(&harness, "abc");
    assert_eq!(harness.audio.take(), [AudioCall::Load(uid, 0)]);
    harness.controller.handle(Command::Next);
    harness.controller.handle(Command::Previous);
    harness.audio.take();
    harness.handle_next_command().await;
    assert_eq!(harness.audio.take(), [AudioCall::Load(uid, 0)]);
    assert_eq!(fake.calls(), 2);
}

#[tokio::test]
async fn a_failing_track_is_reported_and_skipped() {
    let fake = FakeYtDlp::with_body("echo 'ERROR: Video unavailable' >&2\nexit 1").await;
    let mut harness = fake.harness(false, false);
    let song = QueueTrack::from(&test_songs(1, 100_000)[0]);
    play_tracks(&mut harness, vec![youtube_track("bad"), song]);
    harness.handle_next_command().await;
    let song_uid = harness.uid_of("s0").unwrap();
    assert_eq!(harness.audio.take(), [AudioCall::Load(song_uid, 0)]);
    assert_eq!(
        harness.ui.errors(),
        ["Could not play \"Video bad\" from YouTube: yt-dlp failed: Video unavailable"]
    );
}

#[tokio::test]
async fn missing_yt_dlp_asks_for_an_install_and_stops() {
    let fake = FakeYtDlp::missing().await;
    let mut harness = fake.harness(false, false);
    play_tracks(&mut harness, vec![youtube_track("a"), youtube_track("b")]);
    harness.handle_next_command().await;
    harness.handle_next_command().await;
    assert!(
        harness
            .audio
            .take()
            .iter()
            .all(|call| !matches!(call, AudioCall::Load(..)))
    );
    assert_eq!(
        harness.ui.errors(),
        ["Install yt-dlp to play YouTube tracks"; 2]
    );
    assert!(!harness.controller.is_playing());
    assert!(!harness.controller.is_resolving());
}

#[tokio::test]
async fn a_hanging_yt_dlp_does_not_block_skipping() {
    let fake = FakeYtDlp::with_body("exec sleep 30").await;
    let mut harness = fake.harness(false, false);
    let song = QueueTrack::from(&test_songs(1, 100_000)[0]);
    play_tracks(&mut harness, vec![youtube_track("slow"), song]);
    assert!(harness.controller.is_resolving());
    harness.controller.handle(Command::Next);
    let song_uid = harness.uid_of("s0").unwrap();
    assert_eq!(harness.audio.take(), [AudioCall::Load(song_uid, 0)]);
    assert!(!harness.controller.is_resolving());
}

#[tokio::test]
async fn the_next_youtube_track_is_preloaded_for_gapless_playback() {
    let fake = FakeYtDlp::printing(FUTURE_URL).await;
    let mut harness = fake.harness(true, false);
    let song = QueueTrack::from(&test_songs(1, 100_000)[0]);
    play_tracks(&mut harness, vec![song, youtube_track("next")]);
    harness.start_current();
    harness.audio.take();
    harness.handle_next_command().await;
    let uid = uid_of_video(&harness, "next");
    assert_eq!(harness.audio.take(), [AudioCall::Preload(uid)]);
}

#[tokio::test]
async fn a_failed_preload_is_not_retried_until_the_track_plays() {
    let fake = FakeYtDlp::with_body("exit 1").await;
    let mut harness = fake.harness(true, false);
    let song = QueueTrack::from(&test_songs(1, 100_000)[0]);
    play_tracks(&mut harness, vec![song, youtube_track("next")]);
    harness.start_current();
    harness.handle_next_command().await;
    harness.controller.handle(Command::CycleRepeat);
    harness.controller.handle(Command::CycleRepeat);
    harness.controller.handle(Command::CycleRepeat);
    assert_eq!(fake.calls(), 1);
    assert!(harness.ui.errors().is_empty());
}
