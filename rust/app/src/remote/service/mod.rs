mod client_link;
mod discovery;
mod hosting;
mod transfers;

use std::sync::Arc;
use std::time::Duration;

use feishin_remote::{DeviceInfo, Discovered, Discovery, HostState};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use super::RemoteRequest;
use super::delegate::AppHostDelegate;
use super::target::PlayerTarget;
use super::trust::FileTrust;
use crate::controller::{Command, UiSink};
use crate::session::Session;
use crate::ui::{AppWindow, on_ui, show_status};
use client_link::{ActiveLink, next_link_event};
use discovery::next_devices;

const ANSWER_TIMEOUT: Duration = Duration::from_secs(10);

pub struct ServiceDeps {
    pub commands: mpsc::UnboundedSender<Command>,
    pub session: Session,
    pub window: slint::Weak<AppWindow>,
    pub trust: Arc<FileTrust>,
    pub me: DeviceInfo,
    pub target: PlayerTarget,
    pub mirror: Arc<dyn UiSink>,
    pub host_state: watch::Receiver<HostState>,
    pub delegate: Arc<AppHostDelegate>,
}

pub struct RemoteService {
    deps: ServiceDeps,
    discovery: Option<Discovery>,
    found: Vec<Discovered>,
    hosting: Option<JoinHandle<()>>,
    link: Option<ActiveLink>,
}

impl RemoteService {
    pub fn new(deps: ServiceDeps) -> Self {
        let discovery = Discovery::start()
            .inspect_err(|error| tracing::warn!("finding other players is unavailable: {error}"))
            .ok();
        Self {
            deps,
            discovery,
            found: Vec::new(),
            hosting: None,
            link: None,
        }
    }

    pub async fn run(
        mut self,
        mut requests: mpsc::UnboundedReceiver<RemoteRequest>,
        is_hosting: bool,
    ) {
        let mut devices = self.browse();
        self.set_hosting(is_hosting).await;
        loop {
            tokio::select! {
                request = requests.recv() => match request {
                    Some(request) => self.handle(request).await,
                    None => return,
                },
                found = next_devices(&mut devices) => self.show_devices(found),
                event = next_link_event(&mut self.link) => self.on_link_event(event),
            }
        }
    }

    async fn handle(&mut self, request: RemoteRequest) {
        match request {
            RemoteRequest::Control(id) => {
                self.control(&id).await;
            }
            RemoteRequest::ControlLocal => self.release_link(true),
            RemoteRequest::MovePlaybackTo(id) => self.move_to(&id).await,
            RemoteRequest::MovePlaybackHere => self.move_here().await,
            RemoteRequest::SetHosting(hosting) => self.set_hosting(hosting).await,
            RemoteRequest::ForgetDevices => {
                self.deps.trust.forget_trusted();
                self.status(tr!("Approved devices were forgotten"));
            }
        }
    }

    fn status(&self, message: String) {
        on_ui(&self.deps.window, move |window| {
            show_status(window, &message)
        });
    }
}

#[cfg(test)]
mod tests;
