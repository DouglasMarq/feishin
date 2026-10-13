use std::sync::Arc;

use feishin_core::queue::Queue;
use feishin_storage::queue_store::StoredQueue;
use tokio::sync::mpsc;

use super::{Command, Controller, PersistenceEvent, RestoreState, forward_command};
use crate::store::Store;

pub struct RestoredQueue {
    queue: Queue,
    position_ms: u64,
}

impl RestoredQueue {
    fn from_stored(stored: StoredQueue) -> Self {
        Self {
            queue: Queue::from_snapshot(stored.snapshot),
            position_ms: stored.position_ms,
        }
    }
}

pub async fn load_saved_queue(store: Arc<dyn Store>, commands: mpsc::UnboundedSender<Command>) {
    let restored = tokio::task::spawn_blocking(move || restored_queue(store.as_ref()))
        .await
        .unwrap_or_else(|error| {
            tracing::warn!("loading the saved queue stopped unexpectedly: {error}");
            None
        });
    forward_command(
        &commands,
        Command::Persistence(PersistenceEvent::Restored(restored)),
    );
}

fn restored_queue(store: &dyn Store) -> Option<Box<RestoredQueue>> {
    match store.load_queue() {
        Ok(stored) => stored.map(|stored| Box::new(RestoredQueue::from_stored(stored))),
        Err(error) => {
            tracing::warn!(
                "loading the saved queue failed, starting with an empty queue: {error:#}"
            );
            None
        }
    }
}

impl Controller {
    pub fn expect_saved_queue(&mut self) {
        self.persistence.restore = RestoreState::Awaiting {
            server_queue_wanted: false,
        };
    }

    pub(super) fn restore_saved_queue(&mut self, restored: Option<Box<RestoredQueue>>) {
        let server_queue_wanted = matches!(
            self.persistence.restore,
            RestoreState::Awaiting {
                server_queue_wanted: true
            }
        );
        if let Some(restored) = restored.filter(|_| self.queue_is_untouched()) {
            self.adopt_restored_queue(*restored);
        }
        self.persistence.restore = RestoreState::Settled;
        if server_queue_wanted && self.queue.is_empty() {
            self.fetch_server_queue();
        }
    }

    fn adopt_restored_queue(&mut self, restored: RestoredQueue) {
        self.queue = restored.queue;
        self.position_ms = restored.position_ms;
        self.duration_ms = self.current_duration_ms();
        self.persistence.schedule.mark_saved(self.queue.revision());
        self.queue_sync.forget_song_ids();
        self.publish_all();
        let connected = self.deps.session.get().is_some();
        if let Some(track) = self.current_track().filter(|_| connected) {
            self.update_media_metadata(&track);
        }
    }
}
