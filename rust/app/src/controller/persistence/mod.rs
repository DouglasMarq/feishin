mod local_saves;
mod restore;
mod schedule;
mod shutdown;
mod writer;

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;

pub use restore::{RestoredQueue, load_saved_queue};
pub use schedule::SaveTimer;

use super::{Command, Controller, forward_command};
use crate::store::Store;
use schedule::SaveSchedule;
use writer::QueueFileWriter;

pub enum PersistenceEvent {
    Restored(Option<Box<RestoredQueue>>),
    SaveDue(SaveTimer),
    SyncDue(u64),
    QueueSaveFailed(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RestoreState {
    Settled,
    Awaiting { server_queue_wanted: bool },
}

pub(super) struct QueuePersistence {
    schedule: SaveSchedule,
    writer: QueueFileWriter,
    restore: RestoreState,
}

impl QueuePersistence {
    pub(super) fn new(store: Arc<dyn Store>, commands: mpsc::UnboundedSender<Command>) -> Self {
        Self {
            schedule: SaveSchedule::default(),
            writer: QueueFileWriter::new(store, commands),
            restore: RestoreState::Settled,
        }
    }

    pub(super) fn defer_server_queue_fetch(&mut self) -> bool {
        match self.restore {
            RestoreState::Settled => false,
            RestoreState::Awaiting { .. } => {
                self.restore = RestoreState::Awaiting {
                    server_queue_wanted: true,
                };
                true
            }
        }
    }

    fn is_awaiting_restore(&self) -> bool {
        matches!(self.restore, RestoreState::Awaiting { .. })
    }
}

pub(super) fn send_after(
    commands: &mpsc::UnboundedSender<Command>,
    delay: Duration,
    event: PersistenceEvent,
) {
    let commands = commands.clone();
    let deadline = tokio::time::Instant::now() + delay;
    tokio::spawn(async move {
        tokio::time::sleep_until(deadline).await;
        forward_command(&commands, Command::Persistence(event));
    });
}

impl Controller {
    pub(super) fn on_persistence_event(&mut self, event: PersistenceEvent) {
        match event {
            PersistenceEvent::Restored(restored) => self.restore_saved_queue(restored),
            PersistenceEvent::SaveDue(timer) => self.local_save_due(timer),
            PersistenceEvent::SyncDue(generation) => self.queue_sync_due(generation),
            PersistenceEvent::QueueSaveFailed(revision) => {
                self.persistence.schedule.mark_failed(revision);
            }
        }
    }

    pub(super) fn save_settings(&self) {
        if let Err(error) = self.deps.store.save_settings(&self.settings) {
            tracing::warn!("saving settings failed: {error:#}");
        }
    }

    pub(super) fn persist_credential(&mut self) {
        let Some(server) = self.deps.session.get() else {
            return;
        };
        let Some(credential) = server.credential() else {
            return;
        };
        if self.saved_credential.as_ref() == Some(&credential) {
            return;
        }
        match self
            .deps
            .store
            .set_credential(&server.config().id, &credential)
        {
            Ok(()) => self.saved_credential = Some(credential),
            Err(error) => tracing::warn!("saving credential failed: {error:#}"),
        }
    }

    fn queue_is_untouched(&self) -> bool {
        self.queue.is_empty() && self.queue.revision() == 0
    }

    pub(super) fn saved_queue_is_still_loading(&self) -> bool {
        self.persistence.is_awaiting_restore() && self.queue_is_untouched()
    }
}
