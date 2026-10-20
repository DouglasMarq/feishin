use std::sync::Arc;

use feishin_platform::MediaControlEvent;
use feishin_remote::{ClientLink, ClientMessage, DeviceId, DeviceInfo, connect};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::super::link::{LinkEvent, Mirror, spawn_link};
use super::super::messages::{error_text, failure_text};
use super::super::target::RemoteTarget;
use super::{ANSWER_TIMEOUT, RemoteService};
use crate::controller::{Command, RemoteCommand, forward_command};
use crate::ui::{devices, on_ui};

pub(super) struct ActiveLink {
    device: DeviceInfo,
    messages: mpsc::UnboundedSender<ClientMessage>,
    events: mpsc::UnboundedReceiver<LinkEvent>,
    task: JoinHandle<()>,
}

impl RemoteService {
    pub(super) async fn control(&mut self, id: &DeviceId) -> bool {
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

    pub(super) fn release_link(&mut self, should_republish: bool) {
        let Some(link) = self.link.take() else {
            return;
        };
        link.task.abort();
        self.deps.target.set(None);
        on_ui(&self.deps.window, |window| {
            devices::show_target(window, None)
        });
        if should_republish {
            forward_command(
                &self.deps.commands,
                Command::Remote(RemoteCommand::Republish),
            );
        }
    }

    pub(super) fn on_link_event(&mut self, event: LinkEvent) {
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

    fn show_connecting(&self, is_connecting: bool) {
        on_ui(&self.deps.window, move |window| {
            devices::show_connecting(window, is_connecting)
        });
    }

    pub(super) fn send_to_link(&self, message: ClientMessage) -> bool {
        let sent = self
            .link
            .as_ref()
            .is_some_and(|link| link.messages.send(message).is_ok());
        if !sent {
            self.status(tr!("Not connected to another player"));
        }
        sent
    }

    pub(super) async fn await_answer(&mut self) -> Option<LinkEvent> {
        let link = self.link.as_mut()?;
        tokio::time::timeout(ANSWER_TIMEOUT, link.events.recv())
            .await
            .ok()
            .map(|event| event.unwrap_or(LinkEvent::Closed))
    }

    pub(super) fn linked_name(&self) -> String {
        self.link
            .as_ref()
            .map(|link| link.device.name.clone())
            .unwrap_or_default()
    }
}

pub(super) async fn next_link_event(link: &mut Option<ActiveLink>) -> LinkEvent {
    match link {
        Some(link) => link.events.recv().await.unwrap_or(LinkEvent::Closed),
        None => std::future::pending().await,
    }
}
