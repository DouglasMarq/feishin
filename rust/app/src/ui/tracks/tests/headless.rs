use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use feishin_core::model::{SongSort, SortOrder};
use feishin_storage::settings::UiSettings;
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model, PhysicalSize, SharedString};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{self, UnboundedReceiver};

use super::super::selection_ranges::SelectionRanges;
use super::super::{install, refresh};
use crate::connect::DemoConnector;
use crate::controller::Command;
use crate::covers::DemoCovers;
use crate::demo::FakeServer;
use crate::list_stream::{ListRow, Placement, SongSource};
use crate::session::Session;
use crate::store::JsonStore;
use crate::ui::test_window::{Jobs, claim_routed_platform, routed_runtime};
use crate::ui::{
    AppState, AppWindow, Context, Page, PlayerState, Route, TrackColumn, TrackRow, TracksState,
    nav, playlist_picker,
};

const SIZE: PhysicalSize = PhysicalSize::new(1280, 820);
const TOOLBAR_HEIGHT: f32 = 76.0;
const COLUMN_HEADER_HEIGHT: f32 = 33.0;
const TRACK_ROW_HEIGHT: f32 = 36.0;

fn vertical_center_of_row(row_index: u8) -> f32 {
    TOOLBAR_HEIGHT + COLUMN_HEADER_HEIGHT + TRACK_ROW_HEIGHT * (f32::from(row_index) + 0.5)
}

struct Harness {
    window: Rc<MinimalSoftwareWindow>,
    jobs: Jobs,
    app: AppWindow,
}

impl Harness {
    fn pump(&self, done: impl Fn() -> bool) -> bool {
        let mut buffer =
            vec![PremultipliedRgbaColor::default(); (SIZE.width * SIZE.height) as usize];
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            slint::platform::update_timers_and_animations();
            self.run_queued_jobs();
            self.window.draw_if_needed(|renderer| {
                renderer.render(&mut buffer, SIZE.width as usize);
            });
            if done() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    }

    fn run_queued_jobs(&self) {
        let queued: Vec<_> = self
            .jobs
            .lock()
            .map(|mut jobs| jobs.drain(..).collect())
            .unwrap_or_default();
        for job in queued {
            job();
        }
    }

    fn tracks(&self) -> TracksState<'_> {
        self.app.global::<TracksState>()
    }

    fn row(&self, index: usize) -> TrackRow {
        self.tracks().get_rows().row_data(index).unwrap_or_default()
    }

    fn click(&self, button: PointerEventButton, vertical_position: f32) {
        let position = LogicalPosition::new(400.0, vertical_position);
        self.app
            .window()
            .dispatch_event(WindowEvent::PointerPressed { position, button });
        self.app
            .window()
            .dispatch_event(WindowEvent::PointerReleased { position, button });
    }

    fn press_keys(&self, keys: &[SharedString]) {
        for key in keys {
            let text = key.clone();
            self.app
                .window()
                .dispatch_event(WindowEvent::KeyPressed { text });
        }
        for key in keys.iter().rev() {
            let text = key.clone();
            self.app
                .window()
                .dispatch_event(WindowEvent::KeyReleased { text });
        }
    }
}

fn install_test_platform() -> Option<(Rc<MinimalSoftwareWindow>, Jobs)> {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    let jobs = claim_routed_platform(&window)?;
    Some((window, jobs))
}

fn demo_context(runtime: &Runtime, store_dir: &Path) -> (Context, UnboundedReceiver<Command>) {
    let (commands, received) = mpsc::unbounded_channel();
    let context = Context {
        runtime: runtime.handle().clone(),
        commands,
        session: Session::default(),
        store: Arc::new(JsonStore::new(store_dir)),
        connector: Arc::new(DemoConnector::default()),
        covers: Arc::new(DemoCovers),
        remote: crate::remote::Remote::default(),
    };
    context.session.set(Some(Arc::new(FakeServer::new(600))));
    (context, received)
}

fn start_tracks_view(window: Rc<MinimalSoftwareWindow>, jobs: Jobs, context: &Context) -> Harness {
    let app = AppWindow::new().expect("window");
    let picker = playlist_picker::install(&app, context);
    install(&app, context, &UiSettings::default(), &picker);
    nav::install(&app);
    app.show().expect("show");
    window.set_size(SIZE);
    app.global::<AppState>().set_page(Page::Main);
    refresh(&app);
    Harness { window, jobs, app }
}

fn assert_songs_and_genres_load(harness: &Harness) {
    assert!(harness.pump(|| harness.tracks().get_total() == 600 && harness.row(0).loaded));
    assert!(harness.pump(|| harness.tracks().get_genres().row_count() == 9));
}

fn assert_pointer_selection(harness: &Harness) {
    harness.click(PointerEventButton::Left, vertical_center_of_row(0));
    assert!(harness.row(0).selected);
    harness.click(PointerEventButton::Right, vertical_center_of_row(4));
    assert_eq!(harness.tracks().get_selected_count(), 1);
    assert!(harness.row(4).selected && !harness.row(0).selected);
    assert!(harness.pump(|| true));
}

fn assert_enqueued(
    received: &mut UnboundedReceiver<Command>,
    expected_ranges: &SelectionRanges,
    expected_placement: Placement,
) {
    match received.try_recv() {
        Ok(Command::EnqueueFromList {
            source: SongSource::Library(_),
            ranges,
            placement,
        }) => {
            assert_eq!(&SelectionRanges::from_ranges(ranges), expected_ranges);
            assert_eq!(placement, expected_placement);
        }
        _ => panic!("expected EnqueueFromList"),
    }
}

fn assert_select_all_streams_actions(harness: &Harness, received: &mut UnboundedReceiver<Command>) {
    let state = harness.tracks();
    harness.press_keys(&[Key::Escape.into()]);
    harness.click(PointerEventButton::Left, vertical_center_of_row(0));
    assert!(harness.pump(|| true));
    harness.press_keys(&[Key::Control.into(), "a".into()]);
    assert_eq!(state.get_selected_count(), 600);
    assert!(harness.row(599).selected);
    state.invoke_play_selected();
    assert_enqueued(received, &SelectionRanges::spanning(0..600), Placement::Now);
    state.invoke_queue_selected(true);
    assert_enqueued(
        received,
        &SelectionRanges::spanning(0..600),
        Placement::Next,
    );
    harness.press_keys(&[Key::Escape.into()]);
    assert_eq!(state.get_selected_count(), 0);
    assert!(!harness.row(0).selected);
}

fn assert_app_shortcuts_still_reach_the_player(harness: &Harness) {
    let toggled = Rc::new(std::cell::Cell::new(false));
    let toggled_by_key = Rc::clone(&toggled);
    harness
        .app
        .global::<PlayerState>()
        .on_toggle_pause(move || toggled_by_key.set(true));
    harness.press_keys(&[" ".into()]);
    assert!(toggled.get());
}

fn assert_selection_is_queued(harness: &Harness, received: &mut UnboundedReceiver<Command>) {
    let state = harness.tracks();
    state.invoke_select_row(0, false, false);
    state.invoke_select_row(2, false, true);
    assert_eq!(state.get_selected_count(), 3);
    assert!(harness.row(1).selected);
    state.invoke_queue_selected(false);
    assert!(matches!(received.try_recv(), Ok(Command::AddLast(songs)) if songs.len() == 3));
}

fn assert_go_to_album_navigates(harness: &Harness) {
    let state = harness.tracks();
    let album_id = harness.row(1).album_id;
    state.invoke_context_row(1);
    assert_eq!(state.get_selected_count(), 3);
    state.invoke_select_row(1, false, false);
    state.invoke_go_to_album();
    let navigation = harness.app.global::<AppState>();
    assert_eq!(
        (navigation.get_route(), navigation.get_detail_id()),
        (Route::AlbumDetail, album_id)
    );
    navigation.invoke_back();
}

fn assert_favorite_toggles(harness: &Harness) {
    let favorite = harness.row(1).favorite;
    harness.tracks().invoke_toggle_favorite(1);
    assert!(harness.pump(|| harness.row(1).favorite != favorite));
}

fn assert_filters_reload(harness: &Harness) {
    let state = harness.tracks();
    state.set_filter_favorite(true);
    state.set_genre_index(1);
    state.invoke_filters_changed(false);
    assert!(harness.pump(|| !harness.tracks().get_loading() && harness.row(0).loaded));
    assert_eq!(state.get_selected_count(), 0);
    assert!(state.get_total() > 0 && state.get_total() < 600);
}

fn assert_play_row_uses_filtered_query(
    harness: &Harness,
    received: &mut UnboundedReceiver<Command>,
) {
    harness.tracks().invoke_play_row(0);
    match received.try_recv() {
        Ok(Command::PlayFromList {
            source: SongSource::Library(query),
            song_id,
            row_index,
        }) => {
            assert_eq!(query.favorite, Some(true));
            assert_eq!(query.genre_ids.len(), 1);
            assert_eq!(song_id.as_deref(), Some(harness.row(0).id.as_str()));
            assert_eq!(row_index, ListRow(0));
        }
        _ => panic!("expected PlayFromList"),
    }
}

fn assert_sorting_by_a_column_twice_sorts_it_descending(
    harness: &Harness,
    received: &mut UnboundedReceiver<Command>,
) {
    let state = harness.tracks();
    state.invoke_sort_by(TrackColumn::Year);
    assert_eq!(state.get_sort_column(), TrackColumn::Year);
    assert!(!state.get_sort_desc());
    state.invoke_sort_by(TrackColumn::Year);
    assert!(state.get_sort_desc());
    assert!(harness.pump(|| !harness.tracks().get_loading() && harness.row(0).loaded));
    harness.tracks().invoke_play_row(0);
    match received.try_recv() {
        Ok(Command::PlayFromList {
            source: SongSource::Library(query),
            ..
        }) => assert_eq!((query.sort, query.order), (SongSort::Year, SortOrder::Desc)),
        _ => panic!("expected PlayFromList"),
    }
}

#[test]
fn tracks_view_lists_selects_filters_and_acts_on_demo_songs() {
    let Some((window, jobs)) = install_test_platform() else {
        return;
    };
    let runtime = routed_runtime(1);
    let store_dir = tempfile::tempdir().unwrap();
    let (context, mut received) = demo_context(&runtime, store_dir.path());
    let harness = start_tracks_view(window, jobs, &context);

    assert_songs_and_genres_load(&harness);
    assert_pointer_selection(&harness);
    assert_select_all_streams_actions(&harness, &mut received);
    assert_app_shortcuts_still_reach_the_player(&harness);
    assert_selection_is_queued(&harness, &mut received);
    assert_go_to_album_navigates(&harness);
    assert_favorite_toggles(&harness);
    assert_filters_reload(&harness);
    assert_play_row_uses_filtered_query(&harness, &mut received);
    assert_sorting_by_a_column_twice_sorts_it_descending(&harness, &mut received);
}
