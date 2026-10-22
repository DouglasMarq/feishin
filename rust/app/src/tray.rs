use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use feishin_platform::{ConnectedDevices, Tray, TrayLabels};
use feishin_storage::settings::TraySettings;
use slint::{CloseRequestResponse, ComponentHandle, Model};
use tokio::sync::mpsc::UnboundedSender;

use crate::controller::{Command, forward_command};
use crate::identity::{APP_NAME, MEDIA_IDENTITY};
use crate::ui::{AppWindow, DevicesState, SettingsState};

pub trait ConnectedMenu {
    fn show_connected(&self, connected: Option<&ConnectedDevices>);
}

impl ConnectedMenu for Tray {
    fn show_connected(&self, connected: Option<&ConnectedDevices>) {
        if let Err(error) = Tray::show_connected(self, connected) {
            tracing::warn!("updating the tray menu failed: {error}");
        }
    }
}

pub struct TrayHost<Icon = Tray> {
    start: Box<dyn Fn() -> Option<Icon>>,
    icon: RefCell<Option<Icon>>,
    is_close_to_tray: Cell<bool>,
    connected: RefCell<Option<ConnectedDevices>>,
}

impl TrayHost {
    pub fn new(commands: &UnboundedSender<Command>) -> Rc<Self> {
        let commands = commands.clone();
        Self::with_start(Box::new(move || start_tray(&commands)))
    }
}

impl<Icon: ConnectedMenu + 'static> TrayHost<Icon> {
    fn with_start(start: Box<dyn Fn() -> Option<Icon>>) -> Rc<Self> {
        Rc::new(Self {
            start,
            icon: RefCell::new(None),
            is_close_to_tray: Cell::new(false),
            connected: RefCell::new(None),
        })
    }

    fn apply(&self, settings: TraySettings) {
        self.is_close_to_tray.set(settings.is_close_to_tray);
        let is_shown = self.icon.borrow().is_some();
        match (settings.is_enabled, is_shown) {
            (true, false) => {
                *self.icon.borrow_mut() = (self.start)();
                self.refresh_connected();
            }
            (false, true) => *self.icon.borrow_mut() = None,
            _ => {}
        }
    }

    fn show_connected(&self, connected: Option<ConnectedDevices>) {
        *self.connected.borrow_mut() = connected;
        self.refresh_connected();
    }

    fn refresh_connected(&self) {
        if let Some(icon) = self.icon.borrow().as_ref() {
            icon.show_connected(self.connected.borrow().as_ref());
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

fn connected_devices(window: &AppWindow) -> Option<ConnectedDevices> {
    let controllers = window.global::<DevicesState>().get_controllers();
    let devices: Vec<String> = controllers
        .iter()
        .map(|device| tr!("{} ({})", device.name, device.address))
        .collect();
    (!devices.is_empty()).then(|| ConnectedDevices {
        title: tr!("Connected devices: {}", devices.len()),
        devices,
    })
}

pub fn start(window: &AppWindow, settings: TraySettings, commands: &UnboundedSender<Command>) {
    install(window, &TrayHost::new(commands), settings, commands);
}

fn install<Icon: ConnectedMenu + 'static>(
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
    install_devices(window, host);
}

fn install_devices<Icon: ConnectedMenu + 'static>(window: &AppWindow, host: &Rc<TrayHost<Icon>>) {
    let weak = window.as_weak();
    let host = Rc::clone(host);
    window
        .global::<DevicesState>()
        .on_controllers_shown(move || {
            if let Some(window) = weak.upgrade() {
                host.show_connected(connected_devices(&window));
            }
        });
}

fn install_settings<Icon: ConnectedMenu + 'static>(
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
