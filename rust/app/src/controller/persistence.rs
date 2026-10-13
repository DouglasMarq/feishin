use feishin_storage::queue_store::StoredQueue;
use feishin_storage::settings::UiSettings;
use tokio::sync::oneshot;

use super::Controller;

fn signal_shutdown_finished(done: oneshot::Sender<()>) {
    let _ = done.send(());
}

impl Controller {
    pub(super) fn save_settings(&self) {
        if let Err(error) = self.deps.store.save_settings(&self.settings) {
            tracing::warn!("saving settings failed: {error:#}");
        }
    }

    pub fn persist(&self) {
        self.save_settings();
        let stored = StoredQueue {
            snapshot: self.queue.snapshot(),
            position_ms: self.position_ms,
        };
        if let Err(error) = self.deps.store.save_queue(&stored) {
            tracing::warn!("saving queue failed: {error:#}");
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

    pub(super) fn shut_down(&mut self, ui: UiSettings, done: oneshot::Sender<()>) {
        self.settings.ui = UiSettings {
            follow_current_song: self.settings.ui.follow_current_song,
            ..ui
        };
        self.persist();
        self.persist_credential();
        let Some((server, state)) = self.play_queue_state() else {
            signal_shutdown_finished(done);
            return;
        };
        let queue_sync = self.queue_sync.clone();
        tokio::spawn(async move {
            queue_sync.flush(server, state).await;
            signal_shutdown_finished(done);
        });
    }
}
