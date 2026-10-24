use std::sync::Arc;

use feishin_api::MusicServer;
use i_slint_backend_testing::ElementHandle;
use slint::{ComponentHandle, Model};

use crate::demo::FakeServer;
use crate::ui::test_window::TestWindow;
use crate::ui::{AppState, ArtistDetailState, Route};

fn first_demo_artist_id(server: &FakeServer) -> String {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let album = runtime.block_on(server.album("album-0")).unwrap();
    album.album.album_artists.first().unwrap().id.clone()
}

fn is_shown(view: &TestWindow, label: &str) -> bool {
    ElementHandle::find_by_accessible_label(&view.app, label)
        .next()
        .is_some()
}

#[test]
fn given_an_artist_with_top_songs_its_track_columns_are_labelled() {
    let server = Arc::new(FakeServer::new(200));
    let artist_id = first_demo_artist_id(&server);
    let Some(view) = TestWindow::open(server) else {
        return;
    };

    view.app
        .global::<AppState>()
        .invoke_navigate(Route::ArtistDetail, artist_id.into());

    assert!(view.pump(|| {
        view.app
            .global::<ArtistDetailState>()
            .get_top_songs()
            .row_count()
            > 0
    }));
    assert!(view.pump(|| {
        ["#", "Title", "Plays"]
            .iter()
            .all(|label| is_shown(&view, label))
    }));
}
