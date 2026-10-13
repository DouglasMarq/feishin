use std::cell::RefCell;
use std::rc::Rc;

use feishin_core::model::SearchResults;
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{Platform, PointerEventButton, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model, ModelRc, PhysicalSize, PlatformError};

use super::query::SEARCH_LIMIT;
use super::results::Results;
use super::searches::More;
use super::section::MORE_PAGE;
use super::test_support::numbered_songs;
use crate::ui::{AppState, AppWindow, Page, Route, SearchKind, SearchState};

const SIZE: PhysicalSize = PhysicalSize::new(1280, 820);
const SHOW_MORE_PAGES: usize = 40;
const ROW_HEIGHT: f32 = 36.0;
const PROBED_ROWS: usize = 18;
const FIRST_PROBE: f32 = 100.0;

struct RenderOnlyPlatform {
    window: Rc<MinimalSoftwareWindow>,
}

impl Platform for RenderOnlyPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.window.clone())
    }
}

struct SearchPage {
    window: Rc<MinimalSoftwareWindow>,
    app: AppWindow,
    results: Rc<Results>,
    played: Rc<RefCell<Vec<i32>>>,
}

impl SearchPage {
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

    fn double_click(&self, vertical_position: f32) -> Option<i32> {
        let position = LogicalPosition::new(640.0, vertical_position);
        let button = PointerEventButton::Left;
        for _ in 0..2 {
            let window = self.app.window();
            window.dispatch_event(WindowEvent::PointerPressed { position, button });
            window.dispatch_event(WindowEvent::PointerReleased { position, button });
        }
        self.render();
        self.played.borrow_mut().pop()
    }

    fn probed_rows(&self) -> Vec<Option<i32>> {
        (0..PROBED_ROWS)
            .map(|step| self.double_click(FIRST_PROBE + step as f32 * ROW_HEIGHT))
            .collect()
    }
}

fn show_found_songs(page: &SearchPage) {
    let mut searches = page.results.searches.borrow_mut();
    let generation = searches.begin(Some("song".into()), false).unwrap();
    let found = SearchResults {
        songs: numbered_songs(SEARCH_LIMIT),
        albums: Vec::new(),
        artists: Vec::new(),
    };
    assert!(searches.finish(generation, Some(found), SEARCH_LIMIT));
    drop(searches);
    page.results.show_all(&page.app);
}

fn show_more_songs(page: &SearchPage) {
    let mut searches = page.results.searches.borrow_mut();
    let request = searches.more(SearchKind::Songs).unwrap();
    let count = request.paging.start_index + request.paging.limit;
    let songs = numbered_songs(count)
        .into_iter()
        .skip(request.paging.start_index)
        .collect();
    let applied = searches
        .apply_more(request.generation, request.paging, More::Songs(songs))
        .unwrap();
    drop(searches);
    page.results
        .show_more(&page.app, SearchKind::Songs, &applied);
}

fn record_plays(app: &AppWindow) -> Rc<RefCell<Vec<i32>>> {
    let played = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&played);
    app.global::<SearchState>()
        .on_play(move |position| recorded.borrow_mut().push(position));
    played
}

fn open_search() -> SearchPage {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    let platform = RenderOnlyPlatform {
        window: Rc::clone(&window),
    };
    slint::platform::set_platform(Box::new(platform)).unwrap();
    let app = AppWindow::new().unwrap();
    let results = Rc::new(Results::default());
    app.global::<SearchState>()
        .set_songs(ModelRc::from(Rc::clone(&results)));
    let played = record_plays(&app);
    let navigation = app.global::<AppState>();
    navigation.set_page(Page::Main);
    navigation.set_route(Route::Search);
    app.show().unwrap();
    window.set_size(SIZE);
    let page = SearchPage {
        window,
        app,
        results,
        played,
    };
    page.render();
    page
}

fn assert_consecutive(rows: &[Option<i32>]) {
    let positions: Vec<i32> = rows.iter().flatten().copied().collect();
    assert_eq!(positions.len(), rows.len(), "{rows:?}");
    let consecutive = positions
        .windows(2)
        .all(|pair| pair.first().map(|first| first + 1) == pair.get(1).copied());
    assert!(consecutive, "{rows:?}");
}

#[test]
fn song_rows_follow_the_scroll_after_show_more() {
    let page = open_search();
    show_found_songs(&page);
    for _ in 0..SHOW_MORE_PAGES {
        show_more_songs(&page);
    }
    let shown = SEARCH_LIMIT + SHOW_MORE_PAGES * MORE_PAGE;
    assert_eq!(page.results.row_count(), shown);
    page.render();

    for distance in [20_000.0, -517.0, 1_234.0, 7.0, 108.0, -11.0] {
        page.scroll_down(distance);
        let rows = page.probed_rows();
        assert_consecutive(&rows);
        assert!(rows.first().copied().flatten() > Some(400), "{rows:?}");
    }

    page.scroll_down(1_000_000.0);
    let last_shown = page.probed_rows().into_iter().flatten().max();
    assert_eq!(last_shown, i32::try_from(shown - 1).ok());
}
