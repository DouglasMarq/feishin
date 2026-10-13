use std::cell::RefCell;
use std::rc::Rc;

use feishin_core::model::{Album, AlbumDetail, Song};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{Platform, PointerEventButton, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model, ModelRc, PhysicalSize, PlatformError};

use super::loading::show;
use super::tracks::AlbumTracks;
use super::viewport::{first_visible_row, visible_slots};
use crate::demo::test_song;
use crate::ui::{AlbumDetailState, AppState, AppWindow, Page, Route};

const SIZE: PhysicalSize = PhysicalSize::new(1280, 820);
const BOX_SET_TRACKS: u32 = 2_000;
const TRACKS_PER_DISC: u32 = 500;
const MOST_INSTANTIATED_ROWS: i32 = 26;
const TRACK_HEIGHT: f32 = 36.0;
const PROBED_ROWS: usize = 20;
const FIRST_PROBE: f32 = 10.0;

struct RenderOnlyPlatform {
    window: Rc<MinimalSoftwareWindow>,
}

impl Platform for RenderOnlyPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.window.clone())
    }
}

#[derive(Default)]
struct Viewports {
    slot_counts: Vec<i32>,
    first_rows: Vec<i32>,
}

struct AlbumPage {
    window: Rc<MinimalSoftwareWindow>,
    app: AppWindow,
    viewports: Rc<RefCell<Viewports>>,
    played: Rc<RefCell<Vec<String>>>,
}

impl AlbumPage {
    fn render(&self) {
        let mut buffer =
            vec![PremultipliedRgbaColor::default(); (SIZE.width * SIZE.height) as usize];
        for _ in 0..3 {
            slint::platform::update_timers_and_animations();
            self.window.draw_if_needed(|renderer| {
                renderer.render(&mut buffer, SIZE.width as usize);
            });
        }
    }

    fn scroll_down(&self, distance: f32) {
        self.app
            .window()
            .dispatch_event(WindowEvent::PointerScrolled {
                position: LogicalPosition::new(640.0, 600.0),
                delta_x: 0.0,
                delta_y: -distance,
            });
        self.render();
    }

    fn double_click(&self, vertical_position: f32) -> Option<u32> {
        let position = LogicalPosition::new(640.0, vertical_position);
        let button = PointerEventButton::Left;
        for _ in 0..2 {
            let window = self.app.window();
            window.dispatch_event(WindowEvent::PointerPressed { position, button });
            window.dispatch_event(WindowEvent::PointerReleased { position, button });
        }
        self.render();
        let played = self.played.borrow_mut().pop()?;
        played.strip_prefix("box-track-")?.parse().ok()
    }

    fn probed_tracks(&self) -> Vec<Option<u32>> {
        (0..PROBED_ROWS)
            .map(|step| self.double_click(FIRST_PROBE + step as f32 * TRACK_HEIGHT))
            .collect()
    }

    fn largest_slot_count(&self) -> i32 {
        let viewports = self.viewports.borrow();
        viewports.slot_counts.iter().copied().max().unwrap_or(0)
    }

    fn last_first_row(&self) -> i32 {
        let viewports = self.viewports.borrow();
        viewports.first_rows.last().copied().unwrap_or(0)
    }
}

fn box_set_song(number: u32) -> Song {
    Song {
        disc_number: Some(number / TRACKS_PER_DISC + 1),
        track_number: Some(number % TRACKS_PER_DISC + 1),
        ..test_song(&format!("box-track-{number:04}"), 180_000)
    }
}

fn box_set() -> AlbumDetail {
    AlbumDetail {
        album: Album {
            id: "box-set".into(),
            server_id: "demo".into(),
            name: "Box set".into(),
            album_artists: Vec::new(),
            artists: Vec::new(),
            year: Some(2001),
            genres: Vec::new(),
            song_count: BOX_SET_TRACKS,
            duration_ms: 0,
            cover_art_id: None,
            play_count: 0,
            user_rating: None,
            favorite: false,
            created_at: None,
            last_played_at: None,
        },
        songs: (0..BOX_SET_TRACKS).map(box_set_song).collect(),
    }
}

fn record_viewports(app: &AppWindow) -> Rc<RefCell<Viewports>> {
    let viewports = Rc::new(RefCell::new(Viewports::default()));
    let state = app.global::<AlbumDetailState>();
    let (weak_app, recorded) = (app.as_weak(), Rc::clone(&viewports));
    state.on_visible_slots(move |viewport_height, _row_count| {
        let slots = visible_slots(&weak_app.unwrap(), viewport_height);
        recorded.borrow_mut().slot_counts.push(slots);
        slots
    });
    let (weak_app, recorded) = (app.as_weak(), Rc::clone(&viewports));
    state.on_first_visible_row(move |viewport_top, slots| {
        let first_row = first_visible_row(&weak_app.unwrap(), viewport_top, slots);
        recorded.borrow_mut().first_rows.push(first_row);
        first_row
    });
    viewports
}

fn record_plays(app: &AppWindow) -> Rc<RefCell<Vec<String>>> {
    let played = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&played);
    app.global::<AlbumDetailState>()
        .on_play_track(move |track_id| recorded.borrow_mut().push(track_id.into()));
    played
}

fn open_box_set() -> AlbumPage {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    let platform = RenderOnlyPlatform {
        window: Rc::clone(&window),
    };
    slint::platform::set_platform(Box::new(platform)).unwrap();
    let app = AppWindow::new().unwrap();
    let state = app.global::<AlbumDetailState>();
    state.set_tracks(ModelRc::new(AlbumTracks::default()));
    let viewports = record_viewports(&app);
    let played = record_plays(&app);
    let navigation = app.global::<AppState>();
    navigation.set_page(Page::Main);
    navigation.set_route(Route::AlbumDetail);
    app.show().unwrap();
    window.set_size(SIZE);
    state.set_album_id("box-set".into());
    show(&app, box_set());
    let page = AlbumPage {
        window,
        app,
        viewports,
        played,
    };
    page.render();
    page
}

#[test]
fn box_set_only_instantiates_the_visible_rows() {
    let page = open_box_set();
    let state = page.app.global::<AlbumDetailState>();
    assert_eq!(state.get_tracks().row_count(), 2_004);
    assert!(page.largest_slot_count() > 0);
    assert!(page.largest_slot_count() <= MOST_INSTANTIATED_ROWS);
    assert_eq!(page.last_first_row(), 0);

    page.scroll_down(20_000.0);
    assert!(page.last_first_row() > 400, "{}", page.last_first_row());
    assert!(page.largest_slot_count() <= MOST_INSTANTIATED_ROWS);
}

fn assert_consecutive(tracks: &[Option<u32>]) {
    let numbers: Vec<u32> = tracks.iter().flatten().copied().collect();
    assert_eq!(numbers.len(), tracks.len(), "{tracks:?}");
    let consecutive = numbers
        .windows(2)
        .all(|pair| pair.first().map(|first| first + 1) == pair.get(1).copied());
    assert!(consecutive, "{tracks:?}");
}

#[test]
fn reused_slots_show_the_tracks_under_the_pointer() {
    let page = open_box_set();
    for distance in [20_000.0, -517.0, 1_234.0, 7.0, 108.0, -11.0] {
        page.scroll_down(distance);
        let tracks = page.probed_tracks();
        assert_consecutive(&tracks);
        assert!(
            tracks.first().copied().flatten() > Some(TRACKS_PER_DISC),
            "{tracks:?}"
        );
    }

    page.scroll_down(1_000_000.0);
    let last_shown = page.probed_tracks().into_iter().flatten().max();
    assert_eq!(last_shown, Some(BOX_SET_TRACKS - 1));
}
