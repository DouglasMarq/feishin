use std::sync::Arc;

use feishin_api::{ApiError, MusicServer};
use feishin_core::model::RadioStation;
use feishin_radio_browser::StationQuery;
use slint::{ComponentHandle, Model};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::UnboundedReceiver;

use super::loading::ServerStations;
use super::model::RadioModel;
use super::now_playing::show_now_playing;
use super::view::{RadioView, install_with};
use crate::connect::DemoConnector;
use crate::controller::{Command, RadioCommand, RadioNowPlaying};
use crate::covers::DemoCovers;
use crate::demo::FakeServer;
use crate::radio_directory::{DemoDirectory, StationDirectory};
use crate::session::Session;
use crate::store::JsonStore;
use crate::ui::{AppState, AppWindow, Context, Page, RadioRow, RadioSource, RadioState, Route};

struct RadioPage {
    window: AppWindow,
    view: RadioView,
    server: Arc<FakeServer>,
    received: UnboundedReceiver<Command>,
    runtime: Runtime,
    _dir: tempfile::TempDir,
}

fn favorite() -> RadioStation {
    RadioStation {
        id: "local-1".into(),
        name: "My Station".into(),
        stream_url: "https://my.example.com/stream".into(),
        homepage_url: None,
    }
}

fn open_radio_page() -> RadioPage {
    i_slint_backend_testing::init_no_event_loop();
    let window = AppWindow::new().unwrap_or_else(|error| panic!("window: {error}"));
    let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir: {error}"));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap_or_else(|error| panic!("runtime: {error}"));
    let (commands, received) = tokio::sync::mpsc::unbounded_channel();
    let server = Arc::new(FakeServer::new(20));
    let session = Session::default();
    session.set(Some(server.clone()));
    let ctx = Context {
        rt: runtime.handle().clone(),
        commands,
        session,
        store: Arc::new(JsonStore::new(dir.path())),
        connector: Arc::new(DemoConnector::default()),
        covers: Arc::new(DemoCovers),
        remote: crate::remote::Remote::default(),
    };
    let view = install_with(&window, &ctx, Arc::new(DemoDirectory));
    let app = window.global::<AppState>();
    app.set_page(Page::Main);
    app.set_route(Route::Radio);
    window
        .show()
        .unwrap_or_else(|error| panic!("show: {error}"));
    RadioPage {
        window,
        view,
        server,
        received,
        runtime,
        _dir: dir,
    }
}

impl RadioPage {
    fn finish_loading(&self) {
        let stations = self.runtime.block_on(self.server.radio_stations());
        self.finish_loading_with(stations);
    }

    fn finish_loading_with(&self, stations: feishin_api::Result<Vec<RadioStation>>) {
        let server_stations = ServerStations {
            stations,
            can_manage: true,
        };
        let ticket = self.view.with_model(RadioModel::begin_load);
        self.view
            .apply_loaded(&self.window, ticket, vec![favorite()], server_stations);
    }

    fn finish_search(&self, query: &StationQuery) {
        let found = self.runtime.block_on(DemoDirectory.search(query));
        let generation = self.view.with_model(|model| model.search_generation);
        self.view.apply_search(&self.window, generation, found);
    }

    fn state(&self) -> RadioState<'_> {
        self.window.global::<RadioState>()
    }

    fn stations(&self) -> Vec<RadioRow> {
        self.state().get_stations().iter().collect()
    }

    fn results(&self) -> Vec<RadioRow> {
        self.state().get_results().iter().collect()
    }

    fn next_radio_command(&mut self) -> Option<RadioCommand> {
        while let Ok(command) = self.received.try_recv() {
            if let Command::Radio(radio) = command {
                return Some(radio);
            }
        }
        None
    }
}

#[test]
fn the_view_lists_server_and_saved_stations_and_plays_them() {
    let mut page = open_radio_page();
    assert!(matches!(
        page.received.try_recv(),
        Ok(Command::GetSettings(_))
    ));
    assert!(page.state().get_stations_loading());

    page.finish_loading();

    let stations = page.stations();
    assert_eq!(stations.len(), 5);
    assert_eq!(stations[1].name, "Demo Classic Rock");
    assert_eq!(stations[1].source, RadioSource::Server);
    assert_eq!(stations[4].source, RadioSource::Favorite);
    assert!(stations[4].favorite);
    assert!(page.state().get_can_manage());
    assert!(!page.state().get_stations_loading());

    page.state()
        .invoke_play(RadioSource::Server, "radio-1".into());
    assert!(matches!(
        page.next_radio_command(),
        Some(RadioCommand::Play(station)) if station.name == "Demo Classic Rock"
    ));
    page.state().invoke_stop();
    assert_eq!(page.next_radio_command(), Some(RadioCommand::Stop));

    page.state()
        .invoke_toggle_favorite(RadioSource::Favorite, "local-1".into());
    assert_eq!(
        page.next_radio_command(),
        Some(RadioCommand::RemoveFavorite("local-1".into()))
    );
    assert_eq!(page.stations().len(), 4);
}

#[test]
fn the_directory_search_finds_and_saves_stations() {
    let mut page = open_radio_page();
    page.finish_loading();
    let state = page.state();

    state.invoke_search();
    assert_eq!(
        state.get_results_message(),
        "Enter a name, country or tag to search"
    );

    state.set_search_tag("jazz".into());
    state.invoke_search();
    assert!(state.get_searching());
    page.finish_search(&StationQuery {
        tag: "jazz".into(),
        ..StationQuery::default()
    });
    let results = page.results();
    assert_eq!(results.len(), 2);
    assert!(!page.state().get_searching());
    assert!(!page.state().get_has_more());
    assert_eq!(
        results[0].details,
        "Germany \u{b7} jazz, smooth jazz \u{b7} MP3 128 kbps"
    );
    assert!(!results[0].favorite);

    page.state()
        .invoke_toggle_favorite(RadioSource::Directory, "demo-directory-0".into());
    assert!(matches!(
        page.next_radio_command(),
        Some(RadioCommand::AddFavorite(station)) if station.id == "demo-directory-0"
    ));
    assert!(page.results()[0].favorite);
    assert_eq!(page.stations().len(), 6);

    page.state()
        .invoke_play(RadioSource::Directory, "demo-directory-1".into());
    assert!(matches!(
        page.next_radio_command(),
        Some(RadioCommand::Play(station)) if station.stream_url == "https://directory.demo.invalid/1"
    ));
}

#[test]
fn an_unreachable_directory_is_explained() {
    let page = open_radio_page();
    page.state().set_search_name("anything".into());
    page.state().invoke_search();
    let generation = page.view.with_model(|model| model.search_generation);

    page.view
        .apply_search(&page.window, generation, Err(anyhow::anyhow!("no network")));

    assert!(!page.state().get_searching());
    assert_eq!(
        page.state().get_results_message(),
        "The radio directory could not be reached"
    );
    assert!(
        page.window
            .global::<AppState>()
            .get_status()
            .contains("no network")
    );
}

#[test]
fn the_editor_validates_and_saves_on_this_device() {
    let mut page = open_radio_page();
    page.finish_loading();
    let state = page.state();

    state.invoke_edit(RadioSource::Server, "radio-0".into());
    assert!(state.get_editing());
    assert_eq!(state.get_edit_name(), "Demo Jazz Lounge");
    assert_eq!(state.get_edit_url(), "https://radio.demo.invalid/0");
    state.set_edit_url("ftp://radio.demo.invalid/0".into());
    state.invoke_save_edit(RadioSource::Server);
    assert!(state.get_edit_error().contains("http"));

    state.set_edit_id("".into());
    state.set_edit_name("Night Owl".into());
    state.set_edit_url("https://owl.example.com/live".into());
    state.set_edit_homepage("".into());
    state.invoke_save_edit(RadioSource::Favorite);
    assert!(!page.state().get_editing());
    assert!(matches!(
        page.next_radio_command(),
        Some(RadioCommand::AddFavorite(station))
            if station.id.starts_with("local-") && station.name == "Night Owl" && station.homepage_url.is_none()
    ));
    assert!(page.stations().iter().any(|row| row.name == "Night Owl"));
}

#[test]
fn the_playing_station_is_shown() {
    let page = open_radio_page();
    let now_playing = RadioNowPlaying {
        station_id: "radio-1".into(),
        station_name: "Demo Classic Rock".into(),
        stream_title: Some("Band - Hit".into()),
    };

    show_now_playing(&page.window, Some(&now_playing));
    let state = page.state();
    assert!(state.get_active());
    assert_eq!(state.get_playing_id(), "radio-1");
    assert_eq!(state.get_station_name(), "Demo Classic Rock");
    assert_eq!(state.get_stream_title(), "Band - Hit");

    show_now_playing(&page.window, None);
    assert!(!page.state().get_active());
    assert_eq!(page.state().get_stream_title(), "");
}

#[test]
fn server_changes_without_a_connection_are_explained() {
    let page = open_radio_page();
    page.finish_loading();
    page.view.ctx.session.set(None);
    let state = page.state();
    state.set_delete_name("Demo Jazz Lounge".into());
    state.set_delete_id("radio-0".into());

    state.invoke_delete("radio-0".into());

    assert_eq!(state.get_delete_id(), "");
    assert!(!state.get_busy());
    assert_eq!(
        page.window.global::<AppState>().get_status(),
        "Not connected to a server"
    );
}

#[test]
fn a_slow_load_keeps_a_favorite_saved_meanwhile() {
    let page = open_radio_page();
    let ticket = page.view.with_model(RadioModel::begin_load);
    page.view.keep_favorite(favorite());
    let server_stations = ServerStations {
        stations: Ok(Vec::new()),
        can_manage: false,
    };

    page.view
        .apply_loaded(&page.window, ticket, Vec::new(), server_stations);

    assert!(page.stations().iter().any(|row| row.name == "My Station"));
}

#[test]
fn given_a_server_with_radio_its_stations_are_listed_as_server_radio() {
    let page = open_radio_page();
    page.state().set_server_has_radio(false);

    page.finish_loading();

    assert!(page.state().get_server_has_radio());
}

#[test]
fn given_a_server_without_radio_the_view_explains_it_without_an_error() {
    let page = open_radio_page();

    page.finish_loading_with(Err(ApiError::Unsupported("internet radio stations")));

    assert!(!page.state().get_server_has_radio());
    assert_eq!(page.window.global::<AppState>().get_status(), "");
    assert_eq!(page.stations().len(), 1);
}
