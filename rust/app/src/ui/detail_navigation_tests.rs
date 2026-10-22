use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use slint::ComponentHandle;

use super::{
    AlbumDetailState, AppState, AppWindow, ArtistDetailState, Page, PlaylistDetailState, Route,
};

type Loads = Rc<RefCell<Vec<String>>>;

const FRAME: Duration = Duration::from_millis(16);

fn recorder(loads: &Loads) -> impl Fn(slint::SharedString) + 'static {
    let loads = Rc::clone(loads);
    move |id| loads.borrow_mut().push(id.to_string())
}

fn open_detail(route: Route, detail_id: &str) -> (AppWindow, Loads) {
    i_slint_backend_testing::init_no_event_loop();
    let window = AppWindow::new().unwrap();
    let loads = Loads::default();
    window
        .global::<AlbumDetailState>()
        .on_load(recorder(&loads));
    window
        .global::<ArtistDetailState>()
        .on_load(recorder(&loads));
    window
        .global::<PlaylistDetailState>()
        .on_load(recorder(&loads));
    super::nav::install(&window);
    let app = window.global::<AppState>();
    app.set_page(Page::Main);
    app.set_route(route);
    app.set_detail_id(detail_id.into());
    window.show().unwrap();
    settle();
    (window, loads)
}

fn settle() {
    for _ in 0..3 {
        i_slint_backend_testing::mock_elapsed_time(FRAME);
        slint::platform::update_timers_and_animations();
    }
}

fn leave_for_tracks(window: &AppWindow) {
    window
        .global::<AppState>()
        .invoke_navigate(Route::Tracks, "".into());
    settle();
}

fn assert_only_loaded(route: Route, detail_id: &str) {
    let (window, loads) = open_detail(route, detail_id);
    leave_for_tracks(&window);
    assert_eq!(*loads.borrow(), vec![detail_id.to_owned()]);
}

#[test]
fn given_an_open_album_leaving_it_does_not_load_an_empty_album() {
    assert_only_loaded(Route::AlbumDetail, "album-1");
}

#[test]
fn given_an_open_artist_leaving_it_does_not_load_an_empty_artist() {
    assert_only_loaded(Route::ArtistDetail, "artist-1");
}

#[test]
fn given_an_open_playlist_leaving_it_does_not_load_an_empty_playlist() {
    assert_only_loaded(Route::PlaylistDetail, "playlist-1");
}
