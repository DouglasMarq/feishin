use std::time::Instant;

use feishin_storage::queue_store::{self, PlaybackState};
use tokio::sync::oneshot;

use super::schedule::{SaveKind, SaveTimer};
use super::writer::QueueFile;
use super::{Controller, PersistenceEvent, send_after};

impl Controller {
    pub(in crate::controller) fn schedule_local_save(&mut self) {
        if self.saved_queue_is_still_loading() {
            return;
        }
        let Some(timer) = self.persistence.schedule.plan(self.queue.revision()) else {
            return;
        };
        send_after(
            &self.deps.commands,
            timer.delay(),
            PersistenceEvent::SaveDue(timer),
        );
    }

    pub(super) fn local_save_due(&mut self, timer: SaveTimer) {
        if !self
            .persistence
            .schedule
            .is_due(timer, self.queue.revision())
        {
            return;
        }
        match timer.kind {
            SaveKind::Queue => drop(self.save_queue_file()),
            SaveKind::Playback => drop(self.save_playback_file()),
        }
    }

    pub(super) fn save_changes_now(&mut self) -> Option<oneshot::Receiver<()>> {
        if self.saved_queue_is_still_loading() {
            return None;
        }
        match self
            .persistence
            .schedule
            .has_unsaved_changes(self.queue.revision())
        {
            true => self.save_queue_file(),
            false => Some(self.save_playback_file()),
        }
    }

    fn save_queue_file(&mut self) -> Option<oneshot::Receiver<()>> {
        let started = Instant::now();
        let encoded = queue_store::encode(&self.queue.view(), self.position_ms)
            .inspect_err(|error| tracing::warn!("encoding the queue failed: {error}"))
            .ok()?;
        tracing::debug!(
            bytes = encoded.byte_count(),
            elapsed = ?started.elapsed(),
            "encoded the queue for saving"
        );
        let revision = self.queue.revision();
        self.persistence.schedule.mark_saved(revision);
        Some(
            self.persistence
                .writer
                .write(QueueFile::Queue { encoded, revision }),
        )
    }

    fn save_playback_file(&mut self) -> oneshot::Receiver<()> {
        let playback = PlaybackState::of(&self.queue.view(), self.position_ms);
        self.persistence.writer.write(QueueFile::Playback(playback))
    }
}
