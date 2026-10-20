use std::sync::Arc;

use feishin_storage::settings::{LyricsSourceSetting, ReplayGainSetting};
use slint::{ComponentHandle, Model};
use tokio::sync::mpsc::UnboundedReceiver;

use super::callbacks::install_with;
use super::display::show;
use super::test_support::{device, two_servers};
use super::view::Shared;
use crate::connect::DemoConnector;
use crate::controller::{Command, SettingsSnapshot, YouTubeCommand};
use crate::covers::DemoCovers;
use crate::session::Session;
use crate::store::JsonStore;
use crate::ui::{AppState, AppWindow, Context, Page, ReplayGain, Route, SettingsState};

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
    let context = Context {
        runtime: runtime.handle().clone(),
        commands,
        session: Session::default(),
        store: Arc::new(JsonStore::new(dir.path())),
        connector: Arc::new(DemoConnector::default()),
        covers: Arc::new(DemoCovers),
        remote: crate::remote::Remote::default(),
    };
    let shared = Arc::new(Shared::default());
    install_with(&window, &context, &shared);
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
    assert!(servers[0].is_current && !servers[1].is_current);
    let sources: Vec<_> = state
        .get_lyrics_sources()
        .iter()
        .map(|source| source.name)
        .collect();
    assert_eq!(sources, ["Server", "LRCLIB", "NetEase", "Genius"]);
    let devices: Vec<_> = state.get_devices().iter().collect();
    assert_eq!(devices, ["Autoselect device", "Speakers"]);
    assert_eq!(state.get_crossfade_seconds(), 0);
    assert_eq!(state.get_max_crossfade_seconds(), 12);
    assert!(state.get_is_loaded());
}

fn assert_saves_lyrics_and_playback(
    state: &SettingsState,
    received: &mut UnboundedReceiver<Command>,
) {
    state.invoke_toggle_lyrics_source(3);
    match received.try_recv() {
        Ok(Command::SaveIntegrations(saved)) => {
            assert_eq!(saved.lyrics_sources, LyricsSourceSetting::ALL);
        }
        _ => panic!("expected SaveIntegrations"),
    }

    state.set_replay_gain(ReplayGain::Track);
    state.set_device_index(1);
    state.set_crossfade_seconds(6);
    state.invoke_playback_changed();
    match received.try_recv() {
        Ok(Command::ApplyPlayback(playback)) => {
            assert_eq!(playback.replay_gain, ReplayGainSetting::Track);
            assert_eq!(playback.audio_device.as_deref(), Some("x"));
            assert_eq!(playback.crossfade_seconds, 6);
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

#[test]
fn youtube_settings_are_saved_from_the_form() {
    let mut page = open_settings_page();
    let received = &mut page.received;
    while received.try_recv().is_ok() {}
    let state = page.window.global::<SettingsState>();
    let browsers: Vec<_> = state.get_cookie_browsers().iter().collect();
    assert_eq!(browsers.first().map(|label| label.as_str()), Some("None"));
    assert_eq!(state.get_cookie_browser_index(), 0);

    state.invoke_youtube_changed();
    assert!(received.try_recv().is_err());

    state.set_yt_dlp_path(" /opt/yt-dlp ".into());
    state.set_cookie_browser_index(1);
    let countries: Vec<_> = state.get_chart_countries().iter().collect();
    assert!(
        countries
            .first()
            .is_some_and(|label| label.starts_with("Automatic ("))
    );
    let mexico = countries
        .iter()
        .position(|label| label == "Mexico")
        .and_then(|index| i32::try_from(index).ok())
        .unwrap();
    state.set_chart_country_index(mexico);
    state.invoke_youtube_changed();
    match received.try_recv() {
        Ok(Command::YouTube(YouTubeCommand::SaveSettings(saved))) => {
            assert_eq!(saved.yt_dlp_path.as_deref(), Some("/opt/yt-dlp"));
            assert_eq!(
                saved.cookies_from_browser,
                Some(feishin_youtube::CookieBrowser::Firefox)
            );
            assert_eq!(saved.country.as_deref(), Some("MX"));
        }
        _ => panic!("expected SaveSettings"),
    }
}

#[test]
fn a_link_that_is_not_youtube_is_refused() {
    let page = open_settings_page();
    let state = page.window.global::<SettingsState>();
    state.set_youtube_link("https://example.com/song".into());
    state.invoke_play_youtube_link();
    assert_eq!(
        page.window.global::<AppState>().get_status(),
        "Paste a YouTube or YouTube Music link first"
    );
}

mod tooltips {
    use std::time::Duration;

    use i_slint_backend_testing::{ElementHandle, mock_elapsed_time};
    use slint::platform::WindowEvent;
    use slint::{ComponentHandle, LogicalPosition, LogicalSize};

    use super::open_settings_page;
    use crate::ui::{AppWindow, SettingsState, Tooltip};

    const TALL_WINDOW: LogicalSize = LogicalSize::new(1400.0, 4000.0);
    const PAST_THE_TOOLTIP_DELAY: Duration = Duration::from_millis(500);

    fn element(window: &AppWindow, id: &str, label: &str) -> ElementHandle {
        ElementHandle::find_by_element_id(window, id)
            .find(|element| element.accessible_label().as_deref() == Some(label))
            .unwrap()
    }

    fn shown_tooltip(window: &AppWindow) -> String {
        mock_elapsed_time(Duration::ZERO);
        let tooltip = window.global::<Tooltip>();
        assert!(tooltip.get_is_visible());
        tooltip.get_text().into()
    }

    fn hover(window: &AppWindow, id: &str, label: &str) -> ElementHandle {
        window.window().set_size(TALL_WINDOW);
        let target = element(window, id, label);
        let position = target.absolute_position();
        let size = target.size();
        let center = LogicalPosition::new(
            position.x + size.width / 2.0,
            position.y + size.height / 2.0,
        );
        window
            .window()
            .dispatch_event(WindowEvent::PointerMoved { position: center });
        mock_elapsed_time(Duration::ZERO);
        mock_elapsed_time(PAST_THE_TOOLTIP_DELAY);
        target
    }

    #[test]
    fn given_the_pointer_over_a_setting_switch_it_shows_the_setting_and_its_state() {
        let page = open_settings_page();
        let state = page.window.global::<SettingsState>();
        state.set_is_gapless(true);
        hover(&page.window, "TooltipSwitch::toggle", "Gapless playback");
        assert_eq!(shown_tooltip(&page.window), "Gapless playback: on");
        state.set_is_gapless(false);
        assert_eq!(shown_tooltip(&page.window), "Gapless playback: off");
    }

    #[test]
    fn given_the_pointer_over_a_server_button_it_shows_what_the_button_does() {
        let page = open_settings_page();
        let button = hover(&page.window, "TooltipButton::button", "Add server");
        assert_eq!(shown_tooltip(&page.window), "Sign in to another server");
        assert_eq!(
            button.accessible_description().as_deref(),
            Some("Sign in to another server")
        );
    }
}
