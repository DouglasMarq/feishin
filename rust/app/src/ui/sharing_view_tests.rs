use std::sync::Arc;

use feishin_core::model::{ServerConfig, ServerType};
use slint::ComponentHandle;

use crate::demo::{DemoLibrary, FakeServer};
use crate::ui::test_window::TestWindow;
use crate::ui::{ShareKind, ShareState, TracksState};

const FIRST_DEMO_LINK: &str = "https://share.feishin.invalid/demo-share-1";

fn jellyfin_server() -> FakeServer {
    let config = ServerConfig {
        server_type: ServerType::Jellyfin,
        ..FakeServer::config_for_demo()
    };
    FakeServer::with_config(config, DemoLibrary::with_songs(100))
}

#[test]
fn sharing_an_album_copies_its_public_link() {
    let server = Arc::new(FakeServer::new(600));
    let Some(view) = TestWindow::open(server.clone()) else {
        return;
    };
    let state = view.app.global::<ShareState>();
    assert!(state.get_is_available());
    assert_eq!(state.get_menu_title(), "Share");
    while view.next_command().is_some() {}

    state.invoke_share(ShareKind::Album, "album-3".into(), "Blue Train".into());

    assert!(view.pump(|| view.copied_text().is_some()));
    assert_eq!(view.copied_text().as_deref(), Some(FIRST_DEMO_LINK));
    assert_eq!(
        view.status(),
        format!("Share link copied to the clipboard: {FIRST_DEMO_LINK}")
    );
    assert!(!state.get_is_busy());
    assert_eq!(server.shared_items(), [vec!["album-3".to_owned()]]);
    assert!(view.next_command().is_none());
}

#[test]
fn selected_tracks_are_shared_together() {
    let server = Arc::new(FakeServer::new(600));
    let Some(view) = TestWindow::open(server.clone()) else {
        return;
    };
    let tracks = view.app.global::<TracksState>();
    assert!(view.pump(|| tracks.get_total() == 600));
    tracks.invoke_select_row(0, false, false);
    tracks.invoke_select_row(2, false, true);
    assert!(view.pump(|| tracks.get_selected_count() == 3));

    view.app
        .global::<ShareState>()
        .invoke_share_selected_tracks();

    assert!(view.pump(|| view.copied_text().is_some()));
    let shared = server.shared_items();
    assert_eq!(shared.len(), 1);
    assert_eq!(shared[0].len(), 3);
}

#[test]
fn a_selection_that_is_not_loaded_explains_how_to_share_it() {
    let server = Arc::new(FakeServer::new(600));
    let Some(view) = TestWindow::open(server.clone()) else {
        return;
    };
    let tracks = view.app.global::<TracksState>();
    assert!(view.pump(|| tracks.get_total() == 600));
    tracks.invoke_select_all();
    assert!(view.pump(|| tracks.get_selected_count() == 600));

    view.app
        .global::<ShareState>()
        .invoke_share_selected_tracks();

    assert!(view.pump(|| true));
    assert_eq!(
        view.status(),
        "Only loaded tracks can be shared: select fewer tracks or scroll through them first"
    );
    assert!(server.shared_items().is_empty());
}

#[test]
fn jellyfin_disables_sharing_and_explains_why() {
    let server = Arc::new(jellyfin_server());
    let Some(view) = TestWindow::open(server.clone()) else {
        return;
    };
    let state = view.app.global::<ShareState>();
    assert!(!state.get_is_available());
    assert_eq!(
        state.get_menu_title(),
        "Share (this server has no public share links)"
    );

    state.invoke_share(ShareKind::Song, "song-1".into(), "One".into());

    assert!(view.pump(|| true));
    assert_eq!(
        view.status(),
        "Cannot share: this server has no public share links"
    );
    assert_eq!(view.copied_text(), None);
    assert!(server.shared_items().is_empty());
}
