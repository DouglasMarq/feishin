use std::time::Duration;

use feishin_storage::settings::UiSettings;
use tokio::sync::oneshot;

use super::{Controller, PersistenceEvent, send_after};

pub(super) const STREAM_DRAIN_DEADLINE: Duration = Duration::from_secs(3);

pub(in crate::controller) struct PendingShutdown {
    ui: UiSettings,
    done: oneshot::Sender<()>,
}

#[derive(Default)]
pub(in crate::controller) enum ShutdownState {
    #[default]
    Running,
    Draining(Box<PendingShutdown>),
    Finished,
}

impl ShutdownState {
    pub(in crate::controller) fn is_finished(&self) -> bool {
        matches!(self, Self::Finished)
    }
}

impl Controller {
    pub(in crate::controller) fn request_shutdown(
        &mut self,
        ui: UiSettings,
        done: oneshot::Sender<()>,
    ) {
        self.deps.session.end_background_work();
        if self.list_streams.is_empty() {
            self.shutdown = ShutdownState::Finished;
            self.shut_down(ui, done);
            return;
        }
        tracing::info!("waiting for list streams to finish loading before saving the queue");
        self.silence_while_draining();
        self.shutdown = ShutdownState::Draining(Box::new(PendingShutdown { ui, done }));
        send_after(
            &self.deps.commands,
            STREAM_DRAIN_DEADLINE,
            PersistenceEvent::StreamDrainDeadline,
        );
    }

    fn silence_while_draining(&self) {
        if let Err(error) = self.deps.audio.set_muted(true) {
            tracing::warn!("muting playback while quitting failed: {error:#}");
        }
    }

    pub(super) fn stream_drain_deadline(&mut self) {
        if matches!(self.shutdown, ShutdownState::Draining(_)) {
            tracing::warn!("list streams did not finish in time, saving the part loaded so far");
            self.list_streams.cancel_all();
        }
    }

    pub(in crate::controller) fn finish_shutdown_once_drained(&mut self) {
        if !matches!(self.shutdown, ShutdownState::Draining(_)) || !self.list_streams.is_empty() {
            return;
        }
        if let ShutdownState::Draining(pending) =
            std::mem::replace(&mut self.shutdown, ShutdownState::Finished)
        {
            self.shut_down(pending.ui, pending.done);
        }
    }
}
