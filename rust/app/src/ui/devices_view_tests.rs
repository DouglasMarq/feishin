use std::net::IpAddr;
use std::sync::Arc;

use feishin_remote::{Controller, DeviceId, DeviceInfo};
use slint::{ComponentHandle, Model};
use tokio::sync::mpsc;

use crate::controller::Command;
use crate::demo::FakeServer;
use crate::remote::{BannedDevice, Remote, RemoteRequest};
use crate::ui::devices::{show_approval, show_banned, show_controllers};
use crate::ui::test_window::TestWindow;
use crate::ui::{DevicesState, SettingsState};

const KITCHEN: [u8; 4] = [192, 168, 0, 7];

fn kitchen() -> DeviceInfo {
    DeviceInfo {
        id: DeviceId("kitchen".into()),
        name: "Kitchen".into(),
    }
}

#[test]
fn given_a_pairing_request_the_dialog_shows_the_device_address() {
    let Some(view) = TestWindow::open(Arc::new(FakeServer::new(10))) else {
        return;
    };
    let state = view.app.global::<DevicesState>();

    show_approval(&view.app, Some(("Kitchen", IpAddr::from([192, 168, 0, 7]))));

    assert!(state.get_is_approval_open());
    assert_eq!(state.get_approval_name(), "Kitchen");
    assert_eq!(state.get_approval_address(), "192.168.0.7");

    show_approval(&view.app, None);

    assert!(!state.get_is_approval_open());
    assert_eq!(state.get_approval_address(), "");
}

#[test]
fn given_a_controller_the_devices_button_shows_it() {
    let Some(view) = TestWindow::open(Arc::new(FakeServer::new(10))) else {
        return;
    };
    let state = view.app.global::<DevicesState>();
    let controller = Controller {
        device: kitchen(),
        address: IpAddr::from(KITCHEN),
    };

    show_controllers(&view.app, &[controller]);

    assert_eq!(state.get_controller_names(), "Kitchen");
    let row = state.get_controllers().row_data(0).unwrap();
    assert_eq!(
        (row.id.as_str(), row.address.as_str()),
        ("kitchen", "192.168.0.7")
    );

    show_controllers(&view.app, &[]);

    assert_eq!(state.get_controller_names(), "");
    assert_eq!(state.get_controllers().row_count(), 0);
}

#[test]
fn given_banned_devices_the_settings_list_them() {
    let Some(view) = TestWindow::open(Arc::new(FakeServer::new(10))) else {
        return;
    };
    let banned = BannedDevice {
        id: DeviceId("kitchen".into()),
        name: "Kitchen".into(),
        address: IpAddr::from(KITCHEN),
    };

    show_banned(&view.app, &[banned]);

    let rows = view.app.global::<DevicesState>().get_banned();
    assert_eq!(rows.row_count(), 1);
    assert_eq!(rows.row_data(0).unwrap().name, "Kitchen");
}

#[test]
fn given_the_device_actions_they_send_their_requests() {
    let (requests, mut received) = mpsc::unbounded_channel();
    let Some(view) =
        TestWindow::open_with_remote(Arc::new(FakeServer::new(10)), Remote::new(requests))
    else {
        return;
    };
    let state = view.app.global::<DevicesState>();

    state.invoke_disconnect_device("kitchen".into());
    state.invoke_ban_device("kitchen".into());
    state.invoke_unban_device("kitchen".into());

    let sent: Vec<RemoteRequest> = std::iter::from_fn(|| received.try_recv().ok()).collect();
    let kitchen = || DeviceId("kitchen".into());
    assert_eq!(
        sent,
        [
            RemoteRequest::Disconnect(kitchen()),
            RemoteRequest::Ban(kitchen()),
            RemoteRequest::Unban(kitchen()),
        ]
    );
}

#[test]
fn given_allow_without_asking_is_turned_on_it_is_saved() {
    let Some(view) = TestWindow::open(Arc::new(FakeServer::new(10))) else {
        return;
    };
    let state = view.app.global::<SettingsState>();

    state.set_is_remote_approval_skipped(true);
    state.invoke_integrations_changed();

    let saved = std::iter::from_fn(|| view.next_command()).find_map(|command| match command {
        Command::SaveIntegrations(integrations) => Some(integrations.is_remote_approval_skipped),
        _ => None,
    });
    assert_eq!(saved, Some(true));
}
