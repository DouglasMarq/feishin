use std::cell::Cell;
use std::rc::Rc;

use feishin_storage::settings::TraySettings;
use slint::ComponentHandle;
use tokio::sync::mpsc::{self, UnboundedReceiver};

use super::{TrayHost, install};
use crate::controller::Command;
use crate::ui::{AppWindow, SettingsState};

const CLOSE_TO_TRAY: TraySettings = TraySettings {
    is_enabled: true,
    is_close_to_tray: true,
};

fn host(starts: &Rc<Cell<u32>>, is_available: bool) -> Rc<TrayHost<()>> {
    let counted = Rc::clone(starts);
    TrayHost::with_start(Box::new(move || {
        counted.set(counted.get() + 1);
        is_available.then_some(())
    }))
}

fn open(host: &Rc<TrayHost<()>>) -> (AppWindow, UnboundedReceiver<Command>) {
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
