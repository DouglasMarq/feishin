use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType, Rgb565Pixel};
use slint::{ComponentHandle, Model, PhysicalSize};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{self, UnboundedReceiver};

use super::install;
use super::model::with_discover;
use crate::connect::DemoConnector;
use crate::controller::{Command, TrackCommand};
use crate::covers::DemoCovers;
use crate::demo::{DemoLibrary, FakeServer};
use crate::discover::DemoDiscover;
use crate::session::Session;
use crate::store::JsonStore;
use crate::ui::test_window::{Jobs, claim_routed_platform, routed_runtime};
use crate::ui::{
    AppState, AppWindow, Context, DiscoverCardKind, DiscoverPageKind, DiscoverSection,
    DiscoverState, Page, Route,
};

const SONGS: usize = 2_400;

type EventQueue = Jobs;

struct Screen {
    software_window: Rc<MinimalSoftwareWindow>,
    queue: EventQueue,
    size: PhysicalSize,
}

impl Screen {
    fn pump(&self) {
        let events: Vec<_> = self
            .queue
            .lock()
            .map(|mut queue| queue.drain(..).collect())
            .unwrap_or_default();
        for event in events {
            event();
        }
        slint::platform::update_timers_and_animations();
        let mut buffer =
            vec![Rgb565Pixel::default(); (self.size.width * self.size.height) as usize];
        self.software_window.draw_if_needed(|renderer| {
            renderer.render(&mut buffer, self.size.width as usize);
        });
        self.software_window.request_redraw();
        std::thread::sleep(Duration::from_millis(5));
    }

    fn wait_until(&self, what: &str, done: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !done() && Instant::now() < deadline {
            self.pump();
        }
        self.pump();
        assert!(done(), "{what}");
    }
}

fn install_headless_platform() -> Option<Screen> {
    let size = PhysicalSize::new(1100, 760);
    let software_window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    software_window.set_size(size);
    let Some(queue) = claim_routed_platform(&software_window) else {
        eprintln!("skipped: another headless test owns the Slint event loop proxy");
        return None;
    };
    Some(Screen {
        software_window,
        queue,
        size,
    })
}

struct Fixture {
    runtime: Runtime,
    window: AppWindow,
    received: UnboundedReceiver<Command>,
    _store_dir: tempfile::TempDir,
}

fn open_discover() -> Fixture {
    let runtime = routed_runtime(2);
    let store_dir = tempfile::tempdir().unwrap();
    let session = Session::default();
    session.set(Some(Arc::new(FakeServer::new(SONGS))));
    let (commands, received) = mpsc::unbounded_channel();
    let context = Context {
        runtime: runtime.handle().clone(),
        commands,
        session,
        store: Arc::new(JsonStore::new(store_dir.path())),
        connector: Arc::new(DemoConnector::default()),
        covers: Arc::new(DemoCovers),
        remote: crate::remote::Remote::default(),
    };
    let window = AppWindow::new().unwrap();
    crate::ui::nav::install(&window);
    install(
        &window,
        &context,
        Arc::new(DemoDiscover::new(DemoLibrary::with_songs(SONGS))),
    );
    window.show().unwrap();
    let app = window.global::<AppState>();
    app.set_page(Page::Main);
    app.invoke_navigate(Route::Discover, "".into());
    Fixture {
        runtime,
        window,
        received,
        _store_dir: store_dir,
    }
}

impl Fixture {
    fn state(&self) -> DiscoverState<'_> {
        self.window.global::<DiscoverState>()
    }

    fn shelf_titles(&self) -> Vec<String> {
        self.state()
            .get_shelves()
            .iter()
            .map(|shelf| shelf.title.to_string())
            .collect()
    }

    fn next_command(&mut self) -> Command {
        let received = &mut self.received;
        self.runtime
            .block_on(async {
                tokio::time::timeout(Duration::from_secs(10), received.recv()).await
            })
            .ok()
            .flatten()
            .unwrap_or_else(|| panic!("no command was sent"))
    }
}

const PICKED_SHELF: usize = 1;

fn for_you_is_shown(fixture: &Fixture, screen: &Screen) {
    screen.wait_until("For you loads with your mixes first", || {
        fixture.shelf_titles() == ["Your mixes", "Picked for you", "Playlists to try"]
    });
    screen.wait_until("the picked songs load with covers", || {
        with_discover(&fixture.window, |model| {
            let cards = model.cards(PICKED_SHELF);
            cards.len() == 11 && cards.first().is_some_and(|card| card.has_cover)
        })
        .unwrap_or(false)
    });
    assert!(fixture.state().get_notice().starts_with("Demo mode"));
    assert!(!fixture.state().get_is_loading());
}

fn playing_a_card_plays_the_shelf_from_it(fixture: &mut Fixture) {
    fixture
        .state()
        .invoke_play_from_card(PICKED_SHELF as i32, 2);
    let Command::Tracks(TrackCommand::PlayNow {
        tracks,
        start_index,
    }) = fixture.next_command()
    else {
        panic!("expected PlayNow");
    };
    assert_eq!(start_index, Some(2));
    assert_eq!(tracks.len(), 11);
    assert_eq!(
        tracks.iter().filter(|track| !track.is_external()).count(),
        10
    );
    assert_eq!(
        tracks.last().map(|track| track.song_id.as_str()),
        Some("yt:demoOnlyOnYouTube")
    );
}

fn mixes_open_as_pages_and_play(fixture: &mut Fixture, screen: &Screen) {
    let mix = fixture
        .state()
        .get_shelves()
        .row_data(0)
        .unwrap()
        .cards
        .row_data(0)
        .unwrap();
    assert_eq!(mix.kind, DiscoverCardKind::Mix);
    assert_eq!(mix.title, "Daily Mix 1");
    fixture
        .window
        .global::<AppState>()
        .invoke_navigate(Route::Discover, mix.id.clone());
    screen.wait_until("the mix opens", || {
        fixture.state().get_songs().row_count() >= 12
    });
    assert_eq!(fixture.state().get_page_kind(), DiscoverPageKind::Mix);
    assert!(!fixture.state().get_has_page_link());
    fixture.state().invoke_play_page();
    let Command::Tracks(TrackCommand::PlayNow { tracks, .. }) = fixture.next_command() else {
        panic!("expected PlayNow");
    };
    assert_eq!(tracks.len(), fixture.state().get_songs().row_count());
    fixture.window.global::<AppState>().invoke_back();
    screen.pump();
}

fn charts_follow_the_country(fixture: &Fixture, screen: &Screen) {
    fixture
        .state()
        .invoke_select_section(DiscoverSection::Charts);
    let germany = fixture
        .state()
        .get_countries()
        .iter()
        .position(|country| country == "Germany")
        .unwrap();
    fixture.state().invoke_select_country(germany as i32);
    screen.wait_until("German charts load", || {
        fixture.shelf_titles() == ["Video charts", "Top songs in Germany"]
    });
    assert_eq!(fixture.state().get_country_index(), germany as i32);
}

fn moods_open_their_playlists(fixture: &Fixture, screen: &Screen) {
    fixture
        .state()
        .invoke_select_section(DiscoverSection::Moods);
    screen.wait_until("moods load", || {
        fixture.state().get_moods().row_count() == 8
    });
    assert!(fixture.shelf_titles().is_empty());
    fixture.state().invoke_select_mood(1);
    screen.wait_until("a mood loads", || {
        fixture.shelf_titles() == ["Focus picks", "More Focus"]
    });
    assert_eq!(fixture.state().get_mood_index(), 1);
}

fn playlists_open_as_pages(fixture: &Fixture, screen: &Screen) {
    let playlist = fixture
        .state()
        .get_shelves()
        .row_data(0)
        .unwrap()
        .cards
        .row_data(0)
        .unwrap();
    fixture
        .window
        .global::<AppState>()
        .invoke_navigate(Route::Discover, playlist.id.clone());
    screen.wait_until("the playlist loads", || {
        fixture.state().get_songs().row_count() == 15
    });
    assert_eq!(fixture.state().get_page_kind(), DiscoverPageKind::Playlist);
    assert!(fixture.state().get_has_page_link());
    screen.wait_until("the page cover loads", || {
        fixture.state().get_has_page_cover()
    });
}

fn page_songs_play_from_the_clicked_row(fixture: &mut Fixture, screen: &Screen) {
    fixture.state().invoke_play_song(3);
    let Command::Tracks(TrackCommand::PlayNow {
        tracks,
        start_index,
    }) = fixture.next_command()
    else {
        panic!("expected PlayNow");
    };
    assert_eq!((tracks.len(), start_index), (15, Some(3)));
    fixture.state().invoke_queue_song(0);
    assert!(matches!(
        fixture.next_command(),
        Command::Tracks(TrackCommand::AddLast(tracks)) if tracks.len() == 1
    ));
    screen.wait_until("the queue message shows", || {
        fixture
            .window
            .global::<AppState>()
            .get_status()
            .ends_with("to the queue")
    });
}

fn search_lists_songs_and_back_returns(fixture: &Fixture, screen: &Screen) {
    fixture.state().invoke_search("river".into());
    assert_eq!(
        fixture.window.global::<AppState>().get_detail_id(),
        "search:river"
    );
    screen.wait_until("search results load", || {
        let state = fixture.state();
        state.get_page_title() == "\"river\""
            && !state.get_is_page_loading()
            && state.get_songs().row_count() > 0
    });
    assert_eq!(
        fixture.state().get_page_kind(),
        DiscoverPageKind::SearchResults
    );
    assert!(!fixture.state().get_has_page_link());
    fixture.window.global::<AppState>().invoke_back();
    screen.pump();
    assert_ne!(
        fixture.window.global::<AppState>().get_detail_id(),
        "search:river"
    );
}

#[test]
fn discover_shows_demo_sections_pages_and_plays_from_the_library_first() {
    let Some(screen) = install_headless_platform() else {
        return;
    };
    let mut fixture = open_discover();
    for_you_is_shown(&fixture, &screen);
    playing_a_card_plays_the_shelf_from_it(&mut fixture);
    mixes_open_as_pages_and_play(&mut fixture, &screen);
    charts_follow_the_country(&fixture, &screen);
    moods_open_their_playlists(&fixture, &screen);
    playlists_open_as_pages(&fixture, &screen);
    page_songs_play_from_the_clicked_row(&mut fixture, &screen);
    search_lists_songs_and_back_returns(&fixture, &screen);
}
