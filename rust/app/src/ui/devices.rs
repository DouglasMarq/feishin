use feishin_remote::{DeviceId, DeviceInfo};
use slint::{ComponentHandle, ModelRc, VecModel};

use super::{AppWindow, Context, DevicesState, RemoteDevice};
use crate::remote::RemoteRequest;

pub fn install(window: &AppWindow, context: &Context) {
    let state = window.global::<DevicesState>();
    let with_device = |request: fn(DeviceId) -> RemoteRequest| {
        let context = context.clone();
        move |id: slint::SharedString| context.remote.request(request(DeviceId(id.to_string())))
    };
    state.on_control_device(with_device(RemoteRequest::Control));
    state.on_move_playback_to(with_device(RemoteRequest::MovePlaybackTo));
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

pub fn show_approval(window: &AppWindow, device_name: Option<&str>) {
    let state = window.global::<DevicesState>();
    state.set_is_approval_open(device_name.is_some());
    state.set_approval_name(device_name.unwrap_or_default().into());
}
