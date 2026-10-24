use std::sync::Arc;

use slint::{ComponentHandle, Model, SharedString};

use crate::demo::FakeServer;
use crate::ui::test_window::TestWindow;
use crate::ui::{AppState, ArtistsState, Route};

fn navigate(view: &TestWindow, route: Route) {
    view.app
        .global::<AppState>()
        .invoke_navigate(route, SharedString::default());
}

fn is_loaded(view: &TestWindow) -> bool {
    let state = view.app.global::<ArtistsState>();
    !state.get_is_loading() && state.get_items().row_count() > 0
}

#[test]
fn given_the_artists_page_is_opened_again_it_loads_the_artists_again() {
    let Some(view) = TestWindow::open(Arc::new(FakeServer::new(200))) else {
        return;
    };
    navigate(&view, Route::Artists);
    assert!(view.pump(|| is_loaded(&view)));
    navigate(&view, Route::Tracks);
    assert!(view.pump(|| true));

    navigate(&view, Route::Artists);

    assert!(view.pump(|| view.app.global::<ArtistsState>().get_is_loading()));
    assert!(view.pump(|| is_loaded(&view)));
}
