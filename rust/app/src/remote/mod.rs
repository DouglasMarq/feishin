mod approval;
mod delegate;
mod gate;
mod host_feed;
mod link;
mod lists;
mod messages;
mod service;
mod target;
mod transfer;
mod trust;

use std::sync::Arc;

use feishin_remote::{DeviceId, DeviceInfo, HostState, default_device_name};
use tokio::sync::{mpsc, watch};

pub use approval::Approvals;
pub use delegate::AppHostDelegate;
pub use gate::LocalGate;
pub use host_feed::HostFeed;
pub use target::PlayerTarget;
pub use trust::BannedDevice;

use crate::app::Mode;
use crate::controller::UiSink;
use crate::ui::{AppWindow, Context};
use service::{RemoteService, ServiceDeps};
use trust::FileTrust;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteRequest {
    Control(DeviceId),
    ControlLocal,
    MovePlaybackTo(DeviceId),
    MovePlaybackHere,
    SetHosting(bool),
    ForgetDevices,
    Disconnect(DeviceId),
    Ban(DeviceId),
    Unban(DeviceId),
}

#[derive(Clone, Default)]
pub struct Remote {
    pub target: PlayerTarget,
    pub approvals: Approvals,
    requests: Option<mpsc::UnboundedSender<RemoteRequest>>,
}

impl Remote {
    pub fn new(requests: mpsc::UnboundedSender<RemoteRequest>) -> Self {
        Self {
            requests: Some(requests),
            ..Self::default()
        }
    }

    pub fn request(&self, request: RemoteRequest) {
        let sent = self
            .requests
            .as_ref()
            .is_some_and(|requests| requests.send(request).is_ok());
        if !sent {
            tracing::debug!("the remote control service is not running");
        }
    }
}

pub struct RemoteStart {
    pub mode: Mode,
    pub is_hosting: bool,
    pub is_asking_skipped: bool,
    pub window: slint::Weak<AppWindow>,
    pub host_state: watch::Receiver<HostState>,
    pub mirror: Arc<dyn UiSink>,
    pub requests: mpsc::UnboundedReceiver<RemoteRequest>,
}

pub fn start(context: &Context, start: RemoteStart) {
    let (trust, name) = match start.mode {
        Mode::Real => (
            FileTrust::load(Arc::clone(&context.store)),
            default_device_name(),
        ),
        Mode::Demo(_) => (
            FileTrust::ephemeral(Arc::clone(&context.store)),
            tr!("{} (demo)", default_device_name()),
        ),
    };
    context
        .remote
        .approvals
        .skip_asking(start.is_asking_skipped);
    let trust = Arc::new(trust);
    let me = DeviceInfo {
        id: trust.device_id(),
        name,
    };
    let delegate = Arc::new(AppHostDelegate {
        commands: context.commands.clone(),
        session: context.session.clone(),
        approvals: context.remote.approvals.clone(),
        window: start.window.clone(),
    });
    let service = RemoteService::new(ServiceDeps {
        commands: context.commands.clone(),
        session: context.session.clone(),
        window: start.window,
        trust,
        me,
        target: context.remote.target.clone(),
        mirror: start.mirror,
        host_state: start.host_state,
        delegate,
    });
    context
        .runtime
        .spawn(service.run(start.requests, start.is_hosting));
}
