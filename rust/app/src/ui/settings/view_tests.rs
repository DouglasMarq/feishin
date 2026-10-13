use std::sync::Arc;

use feishin_storage::settings::ReplayGainSetting;
use slint::{ComponentHandle, Model};
use tokio::sync::mpsc::UnboundedReceiver;

use super::callbacks::install_with;
use super::display::show;
use super::test_support::{device, two_servers};
use super::view::Shared;
use crate::connect::DemoConnector;
use crate::controller::{Command, SettingsSnapshot};
use crate::covers::DemoCovers;
use crate::session::Session;
use crate::store::JsonStore;
use crate::ui::{AppState, AppWindow, Context, Page, Route, SettingsState};

struct SettingsPage {
    window: AppWindow,
    received: UnboundedReceiver<Command>,
    _runtime: tokio::runtime::Runtime,
    _dir: tempfile::TempDir,
}

fn open_settings_page() -> SettingsPage {
    i_slint_backend_testing::init_no_event_loop();
    let window = AppWindow::new().unwrap_or_else(|error| panic!("window: {error}"));
    let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir: {error}"));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap_or_else(|error| panic!("runtime: {error}"));
    let (commands, received) = tokio::sync::mpsc::unbounded_channel();
    let ctx = Context {
        rt: runtime.handle().clone(),
        commands,
        session: Session::default(),
        store: Arc::new(JsonStore::new(dir.path())),
        connector: Arc::new(DemoConnector),
        covers: Arc::new(DemoCovers),
    };
    let shared = Arc::new(Shared::default());
    install_with(&window, &ctx, &shared);
    let app = window.global::<AppState>();
    app.set_page(Page::Main);
    app.set_route(Route::Settings);
    window
        .show()
        .unwrap_or_else(|error| panic!("show: {error}"));
    show(
        &window,
        &shared,
        SettingsSnapshot {
            settings: two_servers(),
            devices: vec![device("auto", "Autoselect device"), device("x", "Speakers")],
        },
    );
    SettingsPage {
        window,
        received,
        _runtime: runtime,
        _dir: dir,
    }
}

fn assert_rendered(state: &SettingsState) {
    let servers: Vec<_> = state.get_servers().iter().collect();
    assert_eq!(servers.len(), 2);
    assert_eq!(servers[0].name, "Demo library");
    assert!(servers[0].current && !servers[1].current);
    let sources: Vec<_> = state
        .get_lyrics_sources()
        .iter()
        .map(|source| source.name)
        .collect();
    assert_eq!(sources, ["Server", "LRCLIB", "NetEase", "Genius"]);
    let devices: Vec<_> = state.get_devices().iter().collect();
    assert_eq!(devices, ["Autoselect device", "Speakers"]);
    assert!(state.get_loaded());
}

fn assert_saves_lyrics_and_playback(
    state: &SettingsState,
    received: &mut UnboundedReceiver<Command>,
) {
    state.invoke_toggle_lyrics_source(3);
    match received.try_recv() {
        Ok(Command::SaveIntegrations(saved)) => {
            assert_eq!(
                saved.lyrics_sources,
                ["server", "lrclib", "netease", "genius"]
            );
        }
        _ => panic!("expected SaveIntegrations"),
    }

    state.set_replay_gain(1);
    state.set_device_index(1);
    state.invoke_playback_changed();
    match received.try_recv() {
        Ok(Command::ApplyPlayback(playback)) => {
            assert_eq!(playback.replay_gain, ReplayGainSetting::Track);
            assert_eq!(playback.audio_device.as_deref(), Some("x"));
        }
        _ => panic!("expected ApplyPlayback"),
    }
}

#[test]
fn settings_view_renders_and_saves_changes() {
    let mut page = open_settings_page();
    let received = &mut page.received;
    let window = &page.window;
    for _ in 0..2 {
        assert!(matches!(received.try_recv(), Ok(Command::GetSettings(_))));
    }
    let state = window.global::<SettingsState>();
    assert_rendered(&state);
    assert_saves_lyrics_and_playback(&state, received);

    state.invoke_remove_server("b".into());
    assert!(matches!(received.try_recv(), Ok(Command::RemoveServer(id)) if id == "b"));
    assert!(matches!(received.try_recv(), Ok(Command::GetSettings(_))));
    state.invoke_log_out();
    assert!(matches!(received.try_recv(), Ok(Command::RemoveServer(id)) if id == "demo"));
    assert_eq!(window.global::<AppState>().get_page(), Page::Login);
}
