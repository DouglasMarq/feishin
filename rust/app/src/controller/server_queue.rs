use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_core::model::ServerPlayQueue;

use super::{Command, Controller};
use crate::play_queue_sync::PlayQueueState;

impl Controller {
    pub(super) fn fetch_server_queue(&self) {
        let Some(server) = self.deps.session.get() else {
            return;
        };
        let request = self.play_request;
        let commands = self.deps.commands.clone();
        tokio::spawn(async move {
            match server.play_queue().await {
                Ok(Some(queue)) if !queue.songs.is_empty() => {
                    let _ = commands.send(Command::RestoreQueue { request, queue });
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
        self.publish_all();
    }

    pub(super) fn play_queue_state(&self) -> Option<(Arc<dyn MusicServer>, PlayQueueState)> {
        let server = self.deps.session.get()?;
        let state = PlayQueueState::of(&self.queue, self.position_ms, &server.config().id)?;
        Some((server, state))
    }

    pub(super) fn schedule_queue_sync(&self) {
        if let Some((server, state)) = self.play_queue_state() {
            self.queue_sync.schedule(server, state);
        }
    }
}
