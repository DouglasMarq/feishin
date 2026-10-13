use feishin_api::Credential;
use feishin_core::queue::{QueueSnapshot, RepeatMode, Uid};
use feishin_player::PlayerEvent;
use feishin_storage::settings::{ColumnSetting, UiSettings};

use super::harness::{
    AudioCall, Harness, start_harness, start_harness_with, test_settings, test_songs,
};
use crate::controller::{Command, UiUpdate};
use crate::demo::FakeServer;
use crate::store::Store;

fn resized_title_column() -> UiSettings {
    UiSettings {
        track_columns: vec![ColumnSetting {
            id: "title".into(),
            width: 411.0,
            visible: true,
        }],
        queue_panel_open: true,
        ..UiSettings::default()
    }
}

fn assert_restored_without_playing(restored: &Harness, snapshot: &QueueSnapshot) {
    assert_eq!(&restored.controller.queue().snapshot(), snapshot);
    assert!(restored.controller.queue().shuffle());
    assert_eq!(restored.controller.queue().repeat(), RepeatMode::All);
    assert_eq!(restored.current_id().as_deref(), Some("s3"));
    assert!(restored.audio.take().is_empty());
}

fn assert_connect_republishes_the_restored_song(restored: &mut Harness) {
    let config = restored
        .store
        .load_settings()
        .ok()
        .and_then(|settings| settings.servers.first().cloned())
        .unwrap_or_else(FakeServer::config_for_demo);
    restored.ui.clear();
    restored.controller.handle(Command::Connected {
        config,
        others: Vec::new(),
    });
    let republished = restored
        .ui
        .any(|update| matches!(update, UiUpdate::NowPlaying(Some(song)) if song.id == "s3"));
    assert!(
        republished,
        "connecting must republish the restored song so its cover loads"
    );
}

#[tokio::test]
async fn settings_and_queue_survive_a_restart() {
    let mut harness = start_harness(test_settings(false, false));
    harness.controller.play_now(test_songs(5, 100_000), "s3");
    harness.start_current();
    harness.controller.handle(Command::ToggleShuffle);
    harness.controller.handle(Command::CycleRepeat);
    harness.controller.handle(Command::SetVolume(33));
    harness.player(PlayerEvent::Position {
        position_ms: 12_345,
        duration_ms: None,
    });
    let snapshot = harness.controller.queue().snapshot();
    let ui = resized_title_column();
    assert!(harness.shutdown_with_ui(ui.clone()).await.is_ok());

    let saved = harness.store.load_settings().unwrap_or_default();
    assert_eq!(saved.playback.volume, 33);
    assert_eq!(saved.ui, ui);

    let mut restored = start_harness_with(saved, 100, Some(harness.temp_dir));
    assert_restored_without_playing(&restored, &snapshot);
    assert_connect_republishes_the_restored_song(&mut restored);

    restored.controller.handle(Command::TogglePause);
    let uid = restored.current_uid().unwrap_or(Uid::MAX);
    assert_eq!(restored.audio.take(), [AudioCall::Load(uid, 12_345)]);
}

#[tokio::test]
async fn refreshed_credential_is_persisted_on_connect_and_shutdown() {
    let mut harness = start_harness(test_settings(false, false));
    let credential = |token: &str| Credential::Navidrome {
        subsonic: "u=demo&s=a&t=b".into(),
        nd_token: Some(token.into()),
    };
    harness.server.set_credential(credential("first"));
    harness.connect();
    assert_eq!(
        harness.store.credential("demo").ok().flatten(),
        Some(credential("first"))
    );

    harness.server.set_credential(credential("refreshed"));
    assert!(harness.shutdown().await.is_ok());
    assert_eq!(
        harness.store.credential("demo").ok().flatten(),
        Some(credential("refreshed"))
    );
}
