use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use feishin_api::MusicServer;
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{Clipboard, EventLoopProxy, Platform, WindowAdapter};
use slint::{ComponentHandle, EventLoopError, PhysicalSize, PlatformError};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{self, UnboundedReceiver};

use crate::connect::DemoConnector;
use crate::controller::Command;
use crate::covers::DemoCovers;
use crate::session::Session;
use crate::store::JsonStore;
use crate::ui::{AppState, AppWindow, Context, Page, login};

pub type Jobs = Arc<Mutex<Vec<Box<dyn FnOnce() + Send>>>>;
type CopiedText = Arc<Mutex<Option<String>>>;

pub const SIZE: PhysicalSize = PhysicalSize::new(1280, 820);
const WORKER_SEPARATOR: char = '#';

static JOBS_BY_TEST: LazyLock<Mutex<HashMap<String, Jobs>>> = LazyLock::new(Mutex::default);
static PROXY_CLAIM: Mutex<bool> = Mutex::new(false);

fn test_name() -> String {
    let current = std::thread::current();
    let name = current.name().unwrap_or_default();
    name.split(WORKER_SEPARATOR)
        .next()
        .unwrap_or_default()
        .to_owned()
}

fn jobs_of_this_test() -> Jobs {
    let mut registry = JOBS_BY_TEST.lock().unwrap();
    Arc::clone(registry.entry(test_name()).or_default())
}

struct RecordingPlatform {
    window: Rc<MinimalSoftwareWindow>,
    clipboard: CopiedText,
    owns_proxy: bool,
}

struct RoutingProxy;

impl EventLoopProxy for RoutingProxy {
    fn quit_event_loop(&self) -> Result<(), EventLoopError> {
        Ok(())
    }

    fn invoke_from_event_loop(
        &self,
        event: Box<dyn FnOnce() + Send>,
    ) -> Result<(), EventLoopError> {
        jobs_of_this_test().lock().unwrap().push(event);
        Ok(())
    }
}

fn claim_platform(window: &Rc<MinimalSoftwareWindow>, clipboard: &CopiedText) -> bool {
    let mut proxy_is_ours = PROXY_CLAIM.lock().unwrap();
    let platform = |owns_proxy| RecordingPlatform {
        window: Rc::clone(window),
        clipboard: Arc::clone(clipboard),
        owns_proxy,
    };
    if *proxy_is_ours {
        return slint::platform::set_platform(Box::new(platform(false))).is_ok();
    }
    *proxy_is_ours = slint::platform::set_platform(Box::new(platform(true))).is_ok();
    *proxy_is_ours
}

impl Platform for RecordingPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.window.clone())
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
        self.owns_proxy
            .then(|| Box::new(RoutingProxy) as Box<dyn EventLoopProxy>)
    }

    fn set_clipboard_text(&self, text: &str, clipboard: Clipboard) {
        if clipboard == Clipboard::DefaultClipboard {
            *self.clipboard.lock().unwrap() = Some(text.to_owned());
        }
    }

    fn clipboard_text(&self, clipboard: Clipboard) -> Option<String> {
        (clipboard == Clipboard::DefaultClipboard)
            .then(|| self.clipboard.lock().unwrap().clone())
            .flatten()
    }
}

pub fn claim_routed_platform(window: &Rc<MinimalSoftwareWindow>) -> Option<Jobs> {
    claim_platform(window, &CopiedText::default()).then(jobs_of_this_test)
}

pub fn routed_runtime(worker_threads: usize) -> Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_threads)
        .thread_name(format!("{}{WORKER_SEPARATOR}worker", test_name()))
        .enable_all()
        .build()
        .unwrap()
}

pub fn pump_until(
    window: &MinimalSoftwareWindow,
    jobs: &Jobs,
    timeout: Duration,
    done: impl Fn() -> bool,
) -> bool {
    let mut buffer = vec![PremultipliedRgbaColor::default(); (SIZE.width * SIZE.height) as usize];
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        slint::platform::update_timers_and_animations();
        let queued: Vec<_> = jobs.lock().unwrap().drain(..).collect();
        for job in queued {
            job();
        }
        window.draw_if_needed(|renderer| {
            renderer.render(&mut buffer, SIZE.width as usize);
        });
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

pub struct TestWindow {
    pub app: AppWindow,
    commands: RefCell<UnboundedReceiver<Command>>,
    window: Rc<MinimalSoftwareWindow>,
    jobs: Jobs,
    clipboard: CopiedText,
    _runtime: Runtime,
    _dir: tempfile::TempDir,
}

impl TestWindow {
    pub fn open(server: Arc<dyn MusicServer>) -> Option<Self> {
        let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        let clipboard = CopiedText::default();
        if !claim_platform(&window, &clipboard) {
            eprintln!("skipped: another headless test owns the Slint event loop proxy");
            return None;
        }
        let jobs = jobs_of_this_test();
        let runtime = routed_runtime(2);
        let dir = tempfile::tempdir().unwrap();
        let (sender, commands) = mpsc::unbounded_channel();
        let ctx = Context {
            rt: runtime.handle().clone(),
            commands: sender,
            session: Session::default(),
            store: Arc::new(JsonStore::new(dir.path())),
            connector: Arc::new(DemoConnector::default()),
            covers: Arc::new(DemoCovers),
        };
        ctx.session.set(Some(Arc::clone(&server)));
        let app = AppWindow::new().unwrap();
        crate::ui::install(&app, &ctx, &Default::default());
        app.show().unwrap();
        window.set_size(SIZE);
        login::enter_main(&app, &server);
        assert_eq!(app.global::<AppState>().get_page(), Page::Main);
        Some(Self {
            app,
            commands: RefCell::new(commands),
            window,
            jobs,
            clipboard,
            _runtime: runtime,
            _dir: dir,
        })
    }

    pub fn pump(&self, done: impl Fn() -> bool) -> bool {
        pump_until(&self.window, &self.jobs, Duration::from_secs(10), done)
    }

    pub fn copied_text(&self) -> Option<String> {
        self.clipboard.lock().unwrap().clone()
    }

    pub fn status(&self) -> String {
        self.app.global::<AppState>().get_status().into()
    }

    pub fn next_command(&self) -> Option<Command> {
        self.commands.borrow_mut().try_recv().ok()
    }
}
