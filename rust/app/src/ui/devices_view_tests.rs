use std::net::IpAddr;
use std::sync::Arc;

use slint::ComponentHandle;

use crate::demo::FakeServer;
use crate::ui::DevicesState;
use crate::ui::devices::show_approval;
use crate::ui::test_window::TestWindow;

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
