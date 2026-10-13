use std::cell::RefCell;
use std::rc::Rc;

use feishin_core::queue::{Queue, Uid};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{Platform, PointerEventButton, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, ModelRc, PhysicalSize, PlatformError};

use super::{QueueModel, set_view};
use crate::demo::test_song;
use crate::ui::{AppState, AppWindow, Page, QueueState, Route, SettingsState};

const SIZE: PhysicalSize = PhysicalSize::new(1280, 820);
const QUEUED_SONGS: usize = 1_000;
const FAR_POSITION: usize = 600;
const PANEL_CENTER_X: f32 = 1_110.0;
const PROBE_STEP: f32 = 24.0;

struct RenderOnlyPlatform {
    window: Rc<MinimalSoftwareWindow>,
}

impl Platform for RenderOnlyPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.window.clone())
    }
}

struct QueuePage {
    window: Rc<MinimalSoftwareWindow>,
    app: AppWindow,
    played: Rc<RefCell<Vec<Uid>>>,
}

impl QueuePage {
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

    fn show(&self, queue: &Queue) {
        set_view(&self.app, queue.view());
        self.render();
    }

    fn double_click(&self, vertical_position: f32) -> Option<Uid> {
        let position = LogicalPosition::new(PANEL_CENTER_X, vertical_position);
        let button = PointerEventButton::Left;
        for _ in 0..2 {
            let window = self.app.window();
            window.dispatch_event(WindowEvent::PointerPressed { position, button });
            window.dispatch_event(WindowEvent::PointerReleased { position, button });
        }
        self.render();
        self.played.borrow_mut().pop()
    }

    fn visible_uids(&self) -> Vec<Uid> {
        let probes = (SIZE.height as f32 / PROBE_STEP) as usize;
        let mut uids: Vec<Uid> = (0..probes)
            .filter_map(|step| self.double_click(step as f32 * PROBE_STEP))
            .collect();
        uids.dedup();
        uids
    }
}

fn record_plays(app: &AppWindow) -> Rc<RefCell<Vec<Uid>>> {
    let played = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&played);
    app.global::<QueueState>().on_play(move |uid| {
        if let Ok(uid) = uid.parse() {
            recorded.borrow_mut().push(uid);
        }
    });
    played
}

fn open_queue_panel(follow_current_song: bool) -> QueuePage {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    let platform = RenderOnlyPlatform {
        window: Rc::clone(&window),
    };
    slint::platform::set_platform(Box::new(platform)).unwrap();
    let app = AppWindow::new().unwrap();
    app.global::<QueueState>()
        .set_rows(ModelRc::from(Rc::new(QueueModel::default())));
    app.global::<SettingsState>()
        .set_follow_current_song(follow_current_song);
    let navigation = app.global::<AppState>();
    navigation.set_page(Page::Main);
    navigation.set_route(Route::Settings);
    navigation.set_queue_open(true);
    let played = record_plays(&app);
    app.show().unwrap();
    window.set_size(SIZE);
    let page = QueuePage {
        window,
        app,
        played,
    };
    page.render();
    page
}

fn long_queue() -> Queue {
    let mut queue = Queue::with_seed(11);
    let songs = (0..QUEUED_SONGS)
        .map(|index| test_song(&format!("s{index}"), 180_000))
        .collect();
    queue.play_now(songs, Some("s0"));
    queue
}

fn uid_at(queue: &Queue, position: usize) -> Uid {
    queue
        .view()
        .item_at(position)
        .map_or(Uid::MAX, |item| item.uid)
}

fn jump_far(page: &QueuePage, queue: &mut Queue) -> (Uid, Uid) {
    let first_uid = uid_at(queue, 0);
    let far_uid = uid_at(queue, FAR_POSITION);
    page.show(queue);
    assert!(page.visible_uids().contains(&first_uid));
    queue.play_uid(far_uid);
    page.show(queue);
    (first_uid, far_uid)
}

#[test]
fn queue_panel_scrolls_to_the_current_song_when_following() {
    let page = open_queue_panel(true);
    let mut queue = long_queue();
    let (first_uid, far_uid) = jump_far(&page, &mut queue);
    let visible = page.visible_uids();
    assert!(visible.contains(&far_uid), "{visible:?}");
    assert!(!visible.contains(&first_uid), "{visible:?}");

    let next_uid = uid_at(&queue, FAR_POSITION + 1);
    queue.play_uid(next_uid);
    page.show(&queue);
    assert_eq!(page.visible_uids(), visible);
}

#[test]
fn queue_panel_keeps_its_scroll_when_not_following() {
    let page = open_queue_panel(false);
    let mut queue = long_queue();
    let (first_uid, far_uid) = jump_far(&page, &mut queue);
    let visible = page.visible_uids();
    assert!(visible.contains(&first_uid), "{visible:?}");
    assert!(!visible.contains(&far_uid), "{visible:?}");
}
