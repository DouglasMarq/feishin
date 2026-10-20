use feishin_platform::MediaControlEvent;
use feishin_remote::{ClientMessage, DeviceId, RemoteFailure, TransferPayload};
use tokio::sync::oneshot;

use super::super::link::LinkEvent;
use super::super::messages::failure_text;
use super::super::transfer::{LocalServer, adapt_payload};
use super::{ANSWER_TIMEOUT, RemoteService};
use crate::controller::{Command, RemoteCommand, forward_command};

impl RemoteService {
    pub(super) async fn move_to(&mut self, id: &DeviceId) {
        let payload = match self.take_local_playback().await {
            Ok(payload) => payload,
            Err(failure) => {
                self.status(tr!("Could not move the music: {}", failure_text(failure)));
                return;
            }
        };
        let was_playing = payload.is_playing;
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

    pub(super) async fn move_here(&mut self) {
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
}
