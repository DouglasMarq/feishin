use std::sync::Arc;
use std::time::Duration;

use feishin_remote::{
    ClientLink, ClientMessage, DeviceId, DeviceInfo, Discovered, Discovery, Host, HostState,
    RemoteFailure, TransferPayload, connect,
};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;

use super::RemoteRequest;
use super::delegate::AppHostDelegate;
use super::link::{LinkEvent, Mirror, spawn_link};
use super::messages::{error_text, failure_text};
use super::target::{PlayerTarget, RemoteTarget};
use super::transfer::{LocalServer, adapt_payload};
use super::trust::FileTrust;
use crate::controller::{Command, RemoteCommand, UiSink, forward_command};
use crate::session::Session;
use crate::ui::{AppWindow, devices, on_ui, show_status};
use feishin_platform::MediaControlEvent;

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

struct ActiveLink {
    device: DeviceInfo,
    messages: mpsc::UnboundedSender<ClientMessage>,
    events: mpsc::UnboundedReceiver<LinkEvent>,
    task: JoinHandle<()>,
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
        hosting: bool,
    ) {
        let mut devices = self.browse();
        self.set_hosting(hosting).await;
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

    fn browse(&self) -> Option<watch::Receiver<Vec<Discovered>>> {
        self.discovery
            .as_ref()?
            .browse(self.deps.me.id.clone())
            .inspect_err(|error| tracing::warn!("browsing for other players failed: {error}"))
            .ok()
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

    fn show_devices(&mut self, found: Vec<Discovered>) {
        tracing::info!("found {} other player(s) on the network", found.len());
        self.found = found;
        let devices: Vec<DeviceInfo> = self
            .found
            .iter()
            .map(|found| found.device.clone())
            .collect();
        on_ui(&self.deps.window, move |window| {
            devices::show_devices(window, &devices)
        });
    }

    async fn control(&mut self, id: &DeviceId) -> bool {
        if self.link.as_ref().is_some_and(|link| link.device.id == *id) {
            return true;
        }
        let Some(found) = self
            .found
            .iter()
            .find(|found| found.device.id == *id)
            .cloned()
        else {
            self.status(tr!("That device is no longer on the network"));
            return false;
        };
        self.release_link(false);
        self.show_connecting(true);
        let connected = connect(found.address, self.deps.me.clone(), self.deps.trust.clone()).await;
        self.show_connecting(false);
        match connected {
            Ok(link) => {
                self.adopt_link(link, found.device);
                true
            }
            Err(error) => {
                self.status(tr!(
                    "Could not connect to {}: {}",
                    found.device.name,
                    error_text(&error)
                ));
                false
            }
        }
    }

    fn adopt_link(&mut self, link: ClientLink, device: DeviceInfo) {
        forward_command(
            &self.deps.commands,
            Command::Media(MediaControlEvent::Pause),
        );
        let (events_sender, events) = mpsc::unbounded_channel();
        let task = spawn_link(
            link.incoming,
            Mirror::new(Arc::clone(&self.deps.mirror)),
            events_sender,
        );
        self.deps.target.set(Some(RemoteTarget {
            messages: link.outgoing.clone(),
        }));
        let shown = device.clone();
        on_ui(&self.deps.window, move |window| {
            devices::show_target(window, Some(&shown))
        });
        self.link = Some(ActiveLink {
            device,
            messages: link.outgoing,
            events,
            task,
        });
    }

    fn release_link(&mut self, republish: bool) {
        let Some(link) = self.link.take() else {
            return;
        };
        link.task.abort();
        self.deps.target.set(None);
        on_ui(&self.deps.window, |window| {
            devices::show_target(window, None)
        });
        if republish {
            forward_command(
                &self.deps.commands,
                Command::Remote(RemoteCommand::Republish),
            );
        }
    }

    fn on_link_event(&mut self, event: LinkEvent) {
        let name = self
            .link
            .as_ref()
            .map(|link| link.device.name.clone())
            .unwrap_or_default();
        match event {
            LinkEvent::Closed => {
                self.release_link(true);
                self.status(tr!("Lost the connection to {}", name));
            }
            LinkEvent::Failed(failure) => {
                self.status(tr!("{} could not do that: {}", name, failure_text(failure)));
            }
            LinkEvent::Transfer(_) | LinkEvent::TransferAccepted => {}
        }
    }

    async fn move_to(&mut self, id: &DeviceId) {
        let payload = match self.take_local_playback().await {
            Ok(payload) => payload,
            Err(failure) => {
                self.status(tr!("Could not move the music: {}", failure_text(failure)));
                return;
            }
        };
        let was_playing = payload.playing;
        if !self.control(id).await {
            self.resume_if(was_playing);
            return;
        }
        let name = self.linked_name();
        let sent = self.send_to_link(ClientMessage::TransferHere {
            payload: Box::new(payload),
        });
        match sent.then_some(()) {
            Some(()) => match self.await_answer().await {
                Some(LinkEvent::TransferAccepted) => self.status(tr!("Now playing on {}", name)),
                Some(LinkEvent::Failed(failure)) => {
                    self.resume_if(was_playing);
                    self.status(tr!(
                        "{} could not take the music: {}",
                        name,
                        failure_text(failure)
                    ));
                }
                other => self.answer_lost(other, &name),
            },
            None => self.resume_if(was_playing),
        }
    }

    async fn move_here(&mut self) {
        let name = self.linked_name();
        if !self.send_to_link(ClientMessage::PullPlayback) {
            return;
        }
        match self.await_answer().await {
            Some(LinkEvent::Transfer(payload)) => self.adopt_here(*payload, &name),
            Some(LinkEvent::Failed(failure)) => {
                self.status(tr!(
                    "Could not move the music from {}: {}",
                    name,
                    failure_text(failure)
                ));
            }
            other => self.answer_lost(other, &name),
        }
    }

    fn adopt_here(&mut self, payload: TransferPayload, name: &str) {
        let local = self
            .deps
            .session
            .get()
            .map(|server| LocalServer::of(server.as_ref()));
        match adapt_payload(payload, local.as_ref()) {
            Ok(adopted) => {
                self.release_link(false);
                let command = Command::Remote(RemoteCommand::AdoptTransfer(Box::new(adopted)));
                forward_command(&self.deps.commands, command);
                self.status(tr!("Moved the music from {} to this computer", name));
            }
            Err(failure) => {
                self.status(tr!(
                    "Could not move the music from {}: {}",
                    name,
                    failure_text(failure)
                ));
            }
        }
    }

    fn answer_lost(&mut self, event: Option<LinkEvent>, name: &str) {
        match event {
            Some(LinkEvent::Closed) => {
                self.release_link(true);
                self.status(tr!("Lost the connection to {}", name));
            }
            _ => self.status(tr!("{} did not answer in time", name)),
        }
    }

    async fn take_local_playback(&self) -> Result<TransferPayload, RemoteFailure> {
        let (reply, replied) = oneshot::channel();
        forward_command(
            &self.deps.commands,
            Command::Remote(RemoteCommand::TakeTransfer(reply)),
        );
        tokio::time::timeout(ANSWER_TIMEOUT, replied)
            .await
            .map_err(|_| RemoteFailure::Busy)?
            .map_err(|_| RemoteFailure::Stopped)?
    }

    fn resume_if(&self, was_playing: bool) {
        if was_playing {
            forward_command(&self.deps.commands, Command::Media(MediaControlEvent::Play));
        }
    }

    fn send_to_link(&self, message: ClientMessage) -> bool {
        let sent = self
            .link
            .as_ref()
            .is_some_and(|link| link.messages.send(message).is_ok());
        if !sent {
            self.status(tr!("Not connected to another player"));
        }
        sent
    }

    async fn await_answer(&mut self) -> Option<LinkEvent> {
        let link = self.link.as_mut()?;
        tokio::time::timeout(ANSWER_TIMEOUT, link.events.recv())
            .await
            .ok()
            .map(|event| event.unwrap_or(LinkEvent::Closed))
    }

    fn linked_name(&self) -> String {
        self.link
            .as_ref()
            .map(|link| link.device.name.clone())
            .unwrap_or_default()
    }

    async fn set_hosting(&mut self, hosting: bool) {
        match (hosting, self.hosting.is_some()) {
            (true, false) => self.start_hosting().await,
            (false, true) => self.stop_hosting(),
            _ => {}
        }
    }

    async fn start_hosting(&mut self) {
        let Some(discovery) = &self.discovery else {
            self.status(tr!(
                "Remote control is unavailable: other players cannot find this one"
            ));
            return;
        };
        let Some((listener, port)) = self.open_listener().await else {
            return;
        };
        if let Err(error) = discovery.advertise(&self.deps.me, port) {
            tracing::warn!("announcing this player failed: {error}");
        }
        let host = Host::new(
            self.deps.me.clone(),
            self.deps.delegate.clone(),
            self.deps.trust.clone(),
            self.deps.host_state.clone(),
        );
        self.hosting = Some(tokio::spawn(host.serve(listener)));
        tracing::info!("other players can control this one on port {port}");
    }

    fn stop_hosting(&mut self) {
        if let Some(task) = self.hosting.take() {
            task.abort();
        }
        if let Some(discovery) = &self.discovery {
            discovery.withdraw();
        }
    }

    fn show_connecting(&self, connecting: bool) {
        on_ui(&self.deps.window, move |window| {
            devices::show_connecting(window, connecting)
        });
    }

    async fn open_listener(&self) -> Option<(TcpListener, u16)> {
        let opened = TcpListener::bind(("0.0.0.0", 0))
            .await
            .and_then(|listener| {
                let port = listener.local_addr()?.port();
                Ok((listener, port))
            });
        match opened {
            Ok(opened) => Some(opened),
            Err(error) => {
                tracing::warn!("listening for remote control failed: {error}");
                self.status(tr!(
                    "Remote control is unavailable: the network port could not be opened"
                ));
                None
            }
        }
    }

    fn status(&self, message: String) {
        on_ui(&self.deps.window, move |window| {
            show_status(window, &message)
        });
    }
}

async fn next_devices(devices: &mut Option<watch::Receiver<Vec<Discovered>>>) -> Vec<Discovered> {
    loop {
        let Some(receiver) = devices.as_mut() else {
            return std::future::pending().await;
        };
        match receiver.changed().await {
            Ok(()) => return receiver.borrow_and_update().clone(),
            Err(_) => *devices = None,
        }
    }
}

async fn next_link_event(link: &mut Option<ActiveLink>) -> LinkEvent {
    match link {
        Some(link) => link.events.recv().await.unwrap_or(LinkEvent::Closed),
        None => std::future::pending().await,
    }
}

#[cfg(test)]
mod tests;
