use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use feishin_storage::settings::UiSettings;
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{EventLoopProxy, Platform, PointerEventButton, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, EventLoopError, LogicalPosition, Model, PhysicalSize, PlatformError};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{self, UnboundedReceiver};

use super::super::{install, refresh};
use crate::connect::DemoConnector;
use crate::controller::Command;
use crate::covers::DemoCovers;
use crate::demo::FakeServer;
use crate::session::Session;
use crate::store::JsonStore;
use crate::ui::{AppState, AppWindow, Context, Page, Route, TrackRow, TracksState, nav};

type Jobs = Arc<Mutex<Vec<Box<dyn FnOnce() + Send>>>>;

const SIZE: PhysicalSize = PhysicalSize::new(1280, 820);

struct TestPlatform {
    window: Rc<MinimalSoftwareWindow>,
    jobs: Jobs,
}

struct JobQueueProxy(Jobs);

impl EventLoopProxy for JobQueueProxy {
    fn quit_event_loop(&self) -> Result<(), EventLoopError> {
        Ok(())
    }

    fn invoke_from_event_loop(
        &self,
        event: Box<dyn FnOnce() + Send>,
    ) -> Result<(), EventLoopError> {
        self.0
            .lock()
            .map_err(|_| EventLoopError::EventLoopTerminated)?
            .push(event);
        Ok(())
    }
}

impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.window.clone())
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
        Some(Box::new(JobQueueProxy(Arc::clone(&self.jobs))))
    }
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
}

fn install_test_platform() -> Option<(Rc<MinimalSoftwareWindow>, Jobs)> {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    let jobs = Jobs::default();
    slint::platform::set_platform(Box::new(TestPlatform {
        window: Rc::clone(&window),
        jobs: Arc::clone(&jobs),
    }))
    .ok()?;
    Some((window, jobs))
}

fn demo_context(runtime: &Runtime) -> (Context, UnboundedReceiver<Command>) {
    let (commands, received) = mpsc::unbounded_channel();
    let ctx = Context {
        rt: runtime.handle().clone(),
        commands,
        session: Session::default(),
        store: Arc::new(JsonStore::new(
            std::env::temp_dir().join("feishin-rs-ui-test"),
        )),
        connector: Arc::new(DemoConnector),
        covers: Arc::new(DemoCovers),
    };
    ctx.session.set(Some(Arc::new(FakeServer::new(600))));
    (ctx, received)
}

fn start_tracks_view(window: Rc<MinimalSoftwareWindow>, jobs: Jobs, ctx: &Context) -> Harness {
    let app = AppWindow::new().expect("window");
    install(&app, ctx, &UiSettings::default());
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
    harness.click(PointerEventButton::Left, 160.0);
    assert!(harness.row(0).selected);
    harness.click(PointerEventButton::Right, 304.0);
    assert_eq!(harness.tracks().get_selected_count(), 1);
    assert!(harness.row(4).selected && !harness.row(0).selected);
    assert!(harness.pump(|| true));
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
        Ok(Command::PlayFromList { query, song_id }) => {
            assert_eq!(query.favorite, Some(true));
            assert_eq!(query.genre_ids.len(), 1);
            assert_eq!(song_id, harness.row(0).id.as_str());
        }
        _ => panic!("expected PlayFromList"),
    }
}

#[test]
fn tracks_view_lists_selects_filters_and_acts_on_demo_songs() {
    let Some((window, jobs)) = install_test_platform() else {
        return;
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("runtime");
    let (ctx, mut received) = demo_context(&runtime);
    let harness = start_tracks_view(window, jobs, &ctx);

    assert_songs_and_genres_load(&harness);
    assert_pointer_selection(&harness);
    assert_selection_is_queued(&harness, &mut received);
    assert_go_to_album_navigates(&harness);
    assert_favorite_toggles(&harness);
    assert_filters_reload(&harness);
    assert_play_row_uses_filtered_query(&harness, &mut received);
}
