use std::cell::{Cell, RefCell};
use std::net::IpAddr;
use std::rc::Rc;

use feishin_platform::ConnectedDevices;
use feishin_remote::{Controller, DeviceId, DeviceInfo};
use feishin_storage::settings::TraySettings;
use slint::ComponentHandle;
use tokio::sync::mpsc::{self, UnboundedReceiver};

use super::{ConnectedMenu, TrayHost, install};
use crate::controller::Command;
use crate::ui::devices::show_controllers;
use crate::ui::{AppWindow, SettingsState};

type Shown = Rc<RefCell<Vec<Option<ConnectedDevices>>>>;

struct FakeIcon(Shown);

impl ConnectedMenu for FakeIcon {
    fn show_connected(&self, connected: Option<&ConnectedDevices>) {
        self.0.borrow_mut().push(connected.cloned());
    }
}

const CLOSE_TO_TRAY: TraySettings = TraySettings {
    is_enabled: true,
    is_close_to_tray: true,
};

fn host(starts: &Rc<Cell<u32>>, is_available: bool) -> Rc<TrayHost<FakeIcon>> {
    showing_host(starts, is_available, &Shown::default())
}

fn showing_host(
    starts: &Rc<Cell<u32>>,
    is_available: bool,
    shown: &Shown,
) -> Rc<TrayHost<FakeIcon>> {
    let counted = Rc::clone(starts);
    let shown = Rc::clone(shown);
    TrayHost::with_start(Box::new(move || {
        counted.set(counted.get() + 1);
        is_available.then(|| FakeIcon(Rc::clone(&shown)))
    }))
}

fn open(host: &Rc<TrayHost<FakeIcon>>) -> (AppWindow, UnboundedReceiver<Command>) {
    i_slint_backend_testing::init_no_event_loop();
    let window = AppWindow::new().unwrap();
    let (commands, received) = mpsc::unbounded_channel();
    install(&window, host, TraySettings::default(), &commands);
    (window, received)
}

#[test]
fn given_close_to_tray_with_an_icon_closing_keeps_the_app_running() {
    let host = host(&Rc::new(Cell::new(0)), true);
    host.apply(CLOSE_TO_TRAY);
    assert!(host.keeps_running_on_close());
}

#[test]
fn given_close_to_tray_without_an_icon_closing_quits() {
    let host = host(&Rc::new(Cell::new(0)), false);
    host.apply(CLOSE_TO_TRAY);
    assert!(!host.keeps_running_on_close());
}

#[test]
fn given_the_default_settings_closing_quits() {
    let host = host(&Rc::new(Cell::new(0)), true);
    host.apply(TraySettings::default());
    assert!(!host.keeps_running_on_close());
}

#[test]
fn given_the_tray_is_turned_off_the_icon_is_removed_and_closing_quits() {
    let host = host(&Rc::new(Cell::new(0)), true);
    host.apply(CLOSE_TO_TRAY);
    host.apply(TraySettings {
        is_enabled: false,
        is_close_to_tray: true,
    });
    assert!(host.icon.borrow().is_none());
    assert!(!host.keeps_running_on_close());
}

#[test]
fn given_the_tray_is_already_shown_applying_again_keeps_the_same_icon() {
    let starts = Rc::new(Cell::new(0));
    let host = host(&starts, true);
    host.apply(TraySettings::default());
    host.apply(CLOSE_TO_TRAY);
    assert_eq!(starts.get(), 1);
}

#[test]
fn given_the_app_starts_the_tray_appears_once_the_event_loop_runs() {
    let starts = Rc::new(Cell::new(0));
    let host = host(&starts, true);
    let _window = open(&host);
    assert_eq!(starts.get(), 0);
    slint::platform::update_timers_and_animations();
    assert_eq!(starts.get(), 1);
}

#[test]
fn given_the_close_to_tray_switch_is_turned_on_it_is_applied_and_saved() {
    let host = host(&Rc::new(Cell::new(0)), true);
    let (window, mut received) = open(&host);
    let state = window.global::<SettingsState>();
    state.set_is_tray_enabled(true);
    state.set_is_close_to_tray(true);
    state.invoke_tray_changed();
    assert!(host.keeps_running_on_close());
    let saved = std::iter::from_fn(|| received.try_recv().ok()).find_map(|command| match command {
        Command::SaveTray(tray) => Some(tray),
        _ => None,
    });
    assert_eq!(saved, Some(CLOSE_TO_TRAY));
}

fn kitchen() -> Controller {
    Controller {
        device: DeviceInfo {
            id: DeviceId("kitchen".into()),
            name: "Kitchen".into(),
        },
        address: IpAddr::from([192, 168, 0, 7]),
    }
}

fn kitchen_connected() -> Option<ConnectedDevices> {
    Some(ConnectedDevices {
        title: "Connected devices: 1".into(),
        devices: vec!["Kitchen (192.168.0.7)".into()],
    })
}

#[test]
fn given_a_device_connects_the_tray_lists_it_with_a_counter() {
    let shown = Shown::default();
    let host = showing_host(&Rc::new(Cell::new(0)), true, &shown);
    let (window, _received) = open(&host);
    slint::platform::update_timers_and_animations();

    show_controllers(&window, &[kitchen()]);

    assert_eq!(shown.borrow().last(), Some(&kitchen_connected()));
}

#[test]
fn given_the_last_device_leaves_the_tray_line_is_removed() {
    let shown = Shown::default();
    let host = showing_host(&Rc::new(Cell::new(0)), true, &shown);
    let (window, _received) = open(&host);
    slint::platform::update_timers_and_animations();
    show_controllers(&window, &[kitchen()]);

    show_controllers(&window, &[]);

    assert_eq!(shown.borrow().last(), Some(&None));
}

#[test]
fn given_the_tray_is_turned_on_later_it_shows_the_connected_devices() {
    let shown = Shown::default();
    let host = showing_host(&Rc::new(Cell::new(0)), true, &shown);
    let (window, _received) = open(&host);
    show_controllers(&window, &[kitchen()]);
    assert!(shown.borrow().is_empty());

    slint::platform::update_timers_and_animations();

    assert_eq!(shown.borrow().last(), Some(&kitchen_connected()));
}
