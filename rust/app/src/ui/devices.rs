use std::net::IpAddr;

use feishin_remote::{Controller, DeviceId, DeviceInfo};
use slint::{ComponentHandle, ModelRc, VecModel};

use super::{AppWindow, ConnectedDevice, Context, DevicesState, RemoteDevice};
use crate::remote::{BannedDevice, RemoteRequest};

pub fn install(window: &AppWindow, context: &Context) {
    let state = window.global::<DevicesState>();
    let with_device = |request: fn(DeviceId) -> RemoteRequest| {
        let context = context.clone();
        move |id: slint::SharedString| context.remote.request(request(DeviceId(id.to_string())))
    };
    state.on_control_device(with_device(RemoteRequest::Control));
    state.on_move_playback_to(with_device(RemoteRequest::MovePlaybackTo));
    state.on_disconnect_device(with_device(RemoteRequest::Disconnect));
    state.on_ban_device(with_device(RemoteRequest::Ban));
    state.on_unban_device(with_device(RemoteRequest::Unban));
    let local_context = context.clone();
    state.on_control_local(move || local_context.remote.request(RemoteRequest::ControlLocal));
    let here_context = context.clone();
    state.on_move_playback_here(move || {
        here_context.remote.request(RemoteRequest::MovePlaybackHere)
    });
    let allow_context = context.clone();
    state.on_allow_device(move || allow_context.remote.approvals.answer(true));
    let deny_context = context.clone();
    state.on_deny_device(move || deny_context.remote.approvals.answer(false));
}

pub fn show_devices(window: &AppWindow, devices: &[DeviceInfo]) {
    let rows: Vec<RemoteDevice> = devices
        .iter()
        .map(|device| RemoteDevice {
            id: device.id.0.as_str().into(),
            name: device.name.as_str().into(),
        })
        .collect();
    window
        .global::<DevicesState>()
        .set_devices(ModelRc::new(VecModel::from(rows)));
}

pub fn show_target(window: &AppWindow, device: Option<&DeviceInfo>) {
    let state = window.global::<DevicesState>();
    state.set_is_controlling(device.is_some());
    state.set_active_id(
        device
            .map(|device| device.id.0.as_str())
            .unwrap_or_default()
            .into(),
    );
    state.set_active_name(
        device
            .map(|device| device.name.as_str())
            .unwrap_or_default()
            .into(),
    );
}

pub fn show_connecting(window: &AppWindow, is_connecting: bool) {
    window
        .global::<DevicesState>()
        .set_is_connecting(is_connecting);
}

pub fn show_approval(window: &AppWindow, request: Option<(&str, IpAddr)>) {
    let state = window.global::<DevicesState>();
    state.set_is_approval_open(request.is_some());
    state.set_approval_name(request.map(|(name, _peer)| name).unwrap_or_default().into());
    state.set_approval_address(
        request
            .map(|(_name, peer)| peer.to_string())
            .unwrap_or_default()
            .into(),
    );
}

fn connected_row(device: &DeviceInfo, address: IpAddr) -> ConnectedDevice {
    ConnectedDevice {
        id: device.id.0.as_str().into(),
        name: device.name.as_str().into(),
        address: address.to_string().into(),
    }
}

pub fn show_controllers(window: &AppWindow, controllers: &[Controller]) {
    let state = window.global::<DevicesState>();
    let names: Vec<&str> = controllers
        .iter()
        .map(|controller| controller.device.name.as_str())
        .collect();
    state.set_controller_names(names.join(", ").into());
    let rows: Vec<ConnectedDevice> = controllers
        .iter()
        .map(|controller| connected_row(&controller.device, controller.address))
        .collect();
    state.set_controllers(ModelRc::new(VecModel::from(rows)));
    state.invoke_controllers_shown();
}

pub fn show_banned(window: &AppWindow, banned: &[BannedDevice]) {
    let rows: Vec<ConnectedDevice> = banned
        .iter()
        .map(|banned| ConnectedDevice {
            id: banned.id.0.as_str().into(),
            name: banned.name.as_str().into(),
            address: banned.address.to_string().into(),
        })
        .collect();
    window
        .global::<DevicesState>()
        .set_banned(ModelRc::new(VecModel::from(rows)));
}
