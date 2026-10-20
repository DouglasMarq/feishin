use feishin_core::queue::{Queue, QueueSnapshot};
use feishin_platform::MediaControlEvent;
use feishin_remote::{RemoteFailure, ServerKey, TransferPayload};
use tokio::sync::oneshot;

use super::Controller;

pub enum RemoteCommand {
    TakeTransfer(oneshot::Sender<Result<TransferPayload, RemoteFailure>>),
    AdoptTransfer(Box<AdoptedQueue>),
    Republish,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AdoptedQueue {
    pub snapshot: QueueSnapshot,
    pub position_ms: u64,
    pub is_playing: bool,
}

impl Controller {
    pub(super) fn on_remote_command(&mut self, command: RemoteCommand) {
        match command {
            RemoteCommand::TakeTransfer(reply) => {
                let taken = self.take_transfer();
                if reply.send(taken).is_err() {
                    tracing::debug!("nobody waited for the transferred queue");
                }
            }
            RemoteCommand::AdoptTransfer(adopted) => self.adopt_transfer(*adopted),
            RemoteCommand::Republish => self.publish_all(),
        }
    }

    fn take_transfer(&mut self) -> Result<TransferPayload, RemoteFailure> {
        if self.is_radio_playing() || self.queue.current().is_none() {
            return Err(RemoteFailure::NothingToTransfer);
        }
        let payload = TransferPayload {
            snapshot: self.queue.snapshot(),
            position_ms: self.position_ms,
            is_playing: self.is_playing,
            server: self
                .deps
                .session
                .get()
                .map(|server| ServerKey::of(server.config())),
        };
        self.on_media_event(MediaControlEvent::Pause);
        Ok(payload)
    }

    fn adopt_transfer(&mut self, adopted: AdoptedQueue) {
        self.leave_radio();
        self.stop();
        self.queue = Queue::from_snapshot(adopted.snapshot);
        self.position_ms = adopted.position_ms;
        self.duration_ms = self.current_duration_ms();
        self.schedule_queue_sync();
        self.publish_all();
        match adopted.is_playing {
            true => self.load_current(adopted.position_ms),
            false => {
                if let Some(track) = self.current_track() {
                    self.update_media_metadata(&track);
                }
            }
        }
    }
}
