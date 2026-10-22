use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use feishin_platform::{Tray, TrayLabels};
use feishin_storage::settings::TraySettings;
use slint::{CloseRequestResponse, ComponentHandle};
use tokio::sync::mpsc::UnboundedSender;

use crate::controller::{Command, forward_command};
use crate::identity::{APP_NAME, MEDIA_IDENTITY};
use crate::ui::{AppWindow, SettingsState};

pub struct TrayHost<Icon = Tray> {
    start: Box<dyn Fn() -> Option<Icon>>,
    icon: RefCell<Option<Icon>>,
    is_close_to_tray: Cell<bool>,
}

impl TrayHost {
    pub fn new(commands: &UnboundedSender<Command>) -> Rc<Self> {
        let commands = commands.clone();
        Self::with_start(Box::new(move || start_tray(&commands)))
    }
}

impl<Icon: 'static> TrayHost<Icon> {
    fn with_start(start: Box<dyn Fn() -> Option<Icon>>) -> Rc<Self> {
        Rc::new(Self {
            start,
            icon: RefCell::new(None),
            is_close_to_tray: Cell::new(false),
        })
    }

    fn apply(&self, settings: TraySettings) {
        self.is_close_to_tray.set(settings.is_close_to_tray);
        let is_shown = self.icon.borrow().is_some();
        match (settings.is_enabled, is_shown) {
            (true, false) => *self.icon.borrow_mut() = (self.start)(),
            (false, true) => *self.icon.borrow_mut() = None,
            _ => {}
        }
    }

    fn keeps_running_on_close(&self) -> bool {
        self.is_close_to_tray.get() && self.icon.borrow().is_some()
    }
}

fn start_tray(commands: &UnboundedSender<Command>) -> Option<Tray> {
    let commands = commands.clone();
    let on_event = Box::new(move |event| forward_command(&commands, Command::Media(event)));
    Tray::new(MEDIA_IDENTITY, &labels(), on_event)
        .inspect_err(|error| tracing::warn!("tray icon unavailable: {error}"))
        .ok()
}

fn labels() -> TrayLabels {
    TrayLabels {
        play_pause: tr!("Play/Pause"),
        next: tr!("Next"),
        previous: tr!("Previous"),
        stop: tr!("Stop"),
        show: tr!("Show {}", APP_NAME),
        quit: tr!("Quit"),
    }
}

pub fn start(window: &AppWindow, settings: TraySettings, commands: &UnboundedSender<Command>) {
    install(window, &TrayHost::new(commands), settings, commands);
}

fn install<Icon: 'static>(
    window: &AppWindow,
    host: &Rc<TrayHost<Icon>>,
    settings: TraySettings,
    commands: &UnboundedSender<Command>,
) {
    let starting = Rc::clone(host);
    slint::Timer::single_shot(Duration::ZERO, move || starting.apply(settings));
    let closing = Rc::clone(host);
    window
        .window()
        .on_close_requested(move || close_response(closing.keeps_running_on_close()));
    install_settings(window, host, commands);
}

fn install_settings<Icon: 'static>(
    window: &AppWindow,
    host: &Rc<TrayHost<Icon>>,
    commands: &UnboundedSender<Command>,
) {
    let weak = window.as_weak();
    let host = Rc::clone(host);
    let commands = commands.clone();
    window.global::<SettingsState>().on_tray_changed(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let tray = tray_settings(&window);
        host.apply(tray);
        forward_command(&commands, Command::SaveTray(tray));
    });
}

fn tray_settings(window: &AppWindow) -> TraySettings {
    let state = window.global::<SettingsState>();
    TraySettings {
        is_enabled: state.get_is_tray_enabled(),
        is_close_to_tray: state.get_is_close_to_tray(),
    }
}

fn close_response(keeps_running: bool) -> CloseRequestResponse {
    if !keeps_running && let Err(error) = slint::quit_event_loop() {
        tracing::debug!("quit: {error}");
    }
    CloseRequestResponse::HideWindow
}

#[cfg(test)]
mod tests;
