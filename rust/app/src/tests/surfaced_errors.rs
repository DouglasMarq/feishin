use feishin_storage::settings::{PlaybackSettings, ReplayGainSetting};

use super::harness::{Harness, start_harness, test_settings, test_songs};
use crate::controller::Command;

const AUDIO_GONE: &str = "the audio output is gone";

fn playing_with_a_broken_output() -> Harness {
    let mut harness = start_harness(test_settings(false, false));
    harness.controller.play_now(test_songs(2, 1000), "s0");
    harness.start_current();
    harness.audio.fail_every_call();
    harness.ui.clear();
    harness
}

fn playback_with(change: impl FnOnce(&mut PlaybackSettings)) -> PlaybackSettings {
    let mut playback = test_settings(false, false).playback;
    change(&mut playback);
    playback
}

#[tokio::test]
async fn given_a_failed_seek_the_status_shows_an_error() {
    let mut harness = playing_with_a_broken_output();
    harness.controller.handle(Command::Seek(500));
    assert_eq!(
        harness.ui.errors(),
        [format!("Could not seek: {AUDIO_GONE}")]
    );
}

#[tokio::test]
async fn given_a_failed_pause_the_status_shows_an_error() {
    let mut harness = playing_with_a_broken_output();
    harness.controller.handle(Command::TogglePause);
    assert_eq!(
        harness.ui.errors(),
        [format!("Could not pause or resume playback: {AUDIO_GONE}")]
    );
    assert!(harness.controller.is_playing());
}

#[tokio::test]
async fn given_a_failed_stop_the_status_shows_an_error() {
    let mut harness = playing_with_a_broken_output();
    harness.controller.handle(Command::Remove(
        harness
            .controller
            .queue()
            .display_order()
            .iter()
            .map(|item| item.uid)
            .collect(),
    ));
    assert!(
        harness
            .ui
            .errors()
            .contains(&format!("Could not stop playback: {AUDIO_GONE}"))
    );
}

#[tokio::test]
async fn given_a_failed_replay_gain_change_the_status_shows_an_error() {
    let mut harness = playing_with_a_broken_output();
    let playback = playback_with(|playback| playback.replay_gain = ReplayGainSetting::Album);
    harness.controller.handle(Command::ApplyPlayback(playback));
    assert_eq!(
        harness.ui.errors(),
        [format!("ReplayGain could not be changed: {AUDIO_GONE}")]
    );
}

#[tokio::test]
async fn given_a_failed_audio_device_switch_the_status_shows_an_error() {
    let mut harness = playing_with_a_broken_output();
    let playback = playback_with(|playback| playback.audio_device = Some("speakers".into()));
    harness.controller.handle(Command::ApplyPlayback(playback));
    assert_eq!(
        harness.ui.errors(),
        [format!(
            "The audio device could not be changed: {AUDIO_GONE}"
        )]
    );
}
