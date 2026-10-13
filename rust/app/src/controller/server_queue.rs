use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_core::model::ServerPlayQueue;

use super::persistence::{PersistenceEvent, send_after};
use super::{Command, Controller, forward_command};
use crate::play_queue_sync::{DEBOUNCE, PlayQueueState};

impl Controller {
    pub(super) fn fetch_server_queue(&mut self) {
        if self.persistence.defer_server_queue_fetch() {
            return;
        }
        let Some(server) = self.deps.session.get() else {
            return;
        };
        let request = self.play_request;
        let commands = self.deps.commands.clone();
        tokio::spawn(async move {
            match server.play_queue().await {
                Ok(Some(queue)) if !queue.songs.is_empty() => {
                    forward_command(&commands, Command::RestoreQueue { request, queue });
                }
                Ok(_) => {}
                Err(error) => tracing::debug!("server play queue: {error}"),
            }
        });
    }

    pub(super) fn restore_queue(&mut self, request: u64, queue: ServerPlayQueue) {
        if request != self.play_request || !self.queue.is_empty() {
            return;
        }
        self.queue.set_shuffle(false);
        self.queue
            .play_now(queue.songs, queue.current_song_id.as_deref());
        self.position_ms = queue.position_ms;
        self.duration_ms = self.current_duration_ms();
        if let Some((_, state)) = self.play_queue_state() {
            self.queue_sync.mark_saved(state);
        }
        self.schedule_local_save();
        self.publish_all();
    }

    pub(super) fn play_queue_state(&mut self) -> Option<(Arc<dyn MusicServer>, PlayQueueState)> {
        let server = self.deps.session.get()?;
        let state = self
            .queue_sync
            .state_of(&self.queue, self.position_ms, &server.config().id)?;
        Some((server, state))
    }

    pub(super) fn schedule_queue_sync(&mut self) {
        self.schedule_local_save();
        let generation = self.queue_sync.next_generation();
        match DEBOUNCE.is_zero() {
            true => self.queue_sync_due(generation),
            false => send_after(
                &self.deps.commands,
                DEBOUNCE,
                PersistenceEvent::SyncDue(generation),
            ),
        }
    }

    pub(super) fn queue_sync_due(&mut self, generation: u64) {
        if !self.queue_sync.is_current(generation) || self.saved_queue_is_still_loading() {
            return;
        }
        if let Some((server, state)) = self.play_queue_state() {
            self.queue_sync.upload(server, generation, state);
        }
    }
}
