use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use feishin_api::MusicServer;
use feishin_core::model::Paging;
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType, Rgb565Pixel};
use slint::platform::{EventLoopProxy, Platform, WindowAdapter};
use slint::{ComponentHandle, EventLoopError, PhysicalSize, PlatformError, SharedString};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{self, UnboundedReceiver};

use super::super::{AppState, AppWindow, Context, HomeAlbum, HomeState, Page, Route};
use super::install;
use super::model::with_home_model;
use super::shelf::{SHELF_SIZE, Shelf};
use crate::connect::DemoConnector;
use crate::controller::Command;
use crate::covers::DemoCovers;
use crate::demo::FakeServer;
use crate::session::Session;
use crate::store::JsonStore;

type EventQueue = Arc<Mutex<Vec<Box<dyn FnOnce() + Send>>>>;

struct QueueingProxy {
    queue: EventQueue,
}

impl EventLoopProxy for QueueingProxy {
    fn quit_event_loop(&self) -> Result<(), EventLoopError> {
        Ok(())
    }

    fn invoke_from_event_loop(
        &self,
        event: Box<dyn FnOnce() + Send>,
    ) -> Result<(), EventLoopError> {
        self.queue
            .lock()
            .map_err(|_| EventLoopError::EventLoopTerminated)?
            .push(event);
        Ok(())
    }
}

struct HeadlessPlatform {
    window: Rc<MinimalSoftwareWindow>,
    queue: EventQueue,
}

impl Platform for HeadlessPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.window.clone())
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
        Some(Box::new(QueueingProxy {
            queue: Arc::clone(&self.queue),
        }))
    }
}

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
    }

    fn pump_briefly(&self) {
        self.pump();
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn install_headless_platform() -> Option<Screen> {
    let size = PhysicalSize::new(1000, 700);
    let software_window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    software_window.set_size(size);
    let queue = EventQueue::default();
    let platform = HeadlessPlatform {
        window: Rc::clone(&software_window),
        queue: Arc::clone(&queue),
    };
    if let Err(error) = slint::platform::set_platform(Box::new(platform)) {
        eprintln!("skipped: another headless test owns the Slint event loop proxy ({error})");
        return None;
    }
    Some(Screen {
        software_window,
        queue,
        size,
    })
}

fn demo_context(
    runtime: &Runtime,
    store_dir: &tempfile::TempDir,
    server: &Arc<FakeServer>,
) -> (Context, UnboundedReceiver<Command>) {
    let session = Session::default();
    session.set(Some(server.clone()));
    let (commands, received) = mpsc::unbounded_channel();
    let ctx = Context {
        rt: runtime.handle().clone(),
        commands,
        session,
        store: Arc::new(JsonStore::new(store_dir.path())),
        connector: Arc::new(DemoConnector),
        covers: Arc::new(DemoCovers),
    };
    (ctx, received)
}

fn open_home(ctx: &Context) -> AppWindow {
    let window = AppWindow::new().unwrap_or_else(|error| panic!("window: {error}"));
    crate::ui::nav::install(&window);
    install(&window, ctx);
    window
        .show()
        .unwrap_or_else(|error| panic!("show: {error}"));
    let app = window.global::<AppState>();
    app.set_page(Page::Main);
    app.invoke_navigate(Route::Home, "".into());
    window
}

fn shelves_loaded(window: &AppWindow) -> bool {
    with_home_model(window, |model| {
        (0..4).all(|shelf| model.cards(shelf).len() == SHELF_SIZE)
            && model.cards(0).first().is_some_and(|card| card.has_cover)
    })
    .unwrap_or(false)
}

fn wait_for_shelves(window: &AppWindow, screen: &Screen) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !shelves_loaded(window) && Instant::now() < deadline {
        screen.pump_briefly();
    }
    for _ in 0..20 {
        screen.pump_briefly();
    }
    assert!(shelves_loaded(window), "home shelves did not load");
}

fn assert_first_shelf_matches_server(cards: &[HomeAlbum], runtime: &Runtime, server: &FakeServer) {
    let expected = runtime
        .block_on(server.album_list(
            &Shelf::RecentlyAdded.query(),
            Paging {
                start_index: 0,
                limit: SHELF_SIZE,
            },
        ))
        .map(|page| page.items)
        .unwrap_or_default();
    let names: Vec<_> = cards.iter().map(|card| card.name.to_string()).collect();
    let expected: Vec<_> = expected.iter().map(|album| album.name.clone()).collect();
    assert_eq!(names, expected);
}

fn assert_only_visible_covers_loaded(window: &AppWindow, cards: &[HomeAlbum]) {
    let covers = |shelf: usize| {
        with_home_model(window, |model| {
            model
                .cards(shelf)
                .iter()
                .filter(|card| card.has_cover)
                .count()
        })
        .unwrap_or_default()
    };
    assert!(
        (1..SHELF_SIZE).contains(&covers(0)),
        "only visible cards load covers"
    );
    assert!(!cards[SHELF_SIZE - 1].has_cover);
    assert_eq!(covers(3), 0, "the off-screen shelf has no covers");
}

fn assert_play_sends_album_songs(
    window: &AppWindow,
    runtime: &Runtime,
    received: &mut UnboundedReceiver<Command>,
    album_id: &SharedString,
) {
    window.global::<HomeState>().invoke_play(album_id.clone());
    let command = runtime
        .block_on(async { tokio::time::timeout(Duration::from_secs(5), received.recv()).await });
    match command {
        Ok(Some(Command::PlaySongs { songs, song_id })) => {
            assert!(song_id.is_none());
            assert!(!songs.is_empty());
            assert!(
                songs
                    .iter()
                    .all(|song| song.album_id.as_deref() == Some(album_id.as_str()))
            );
        }
        _ => panic!("expected PlaySongs"),
    }
}

#[test]
fn home_route_renders_shelves_with_lazy_covers_and_plays_albums() {
    let Some(screen) = install_headless_platform() else {
        return;
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap_or_else(|error| panic!("runtime: {error}"));
    let store_dir = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir: {error}"));
    let server = Arc::new(FakeServer::new(2_400));
    let (ctx, mut received) = demo_context(&runtime, &store_dir, &server);

    let window = open_home(&ctx);
    wait_for_shelves(&window, &screen);

    let cards = with_home_model(&window, |model| model.cards(0)).unwrap_or_default();
    assert_first_shelf_matches_server(&cards, &runtime, &server);
    assert_only_visible_covers_loaded(&window, &cards);
    assert_play_sends_album_songs(&window, &runtime, &mut received, &cards[0].id);
}
