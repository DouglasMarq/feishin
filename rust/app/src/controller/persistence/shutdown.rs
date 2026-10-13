use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_storage::settings::UiSettings;
use tokio::sync::oneshot;

use super::Controller;
use crate::play_queue_sync::{PlayQueueState, PlayQueueUploader};

struct ServerFlush {
    uploader: PlayQueueUploader,
    server: Arc<dyn MusicServer>,
    state: PlayQueueState,
}

fn signal_shutdown_finished(done: oneshot::Sender<()>) {
    if done.send(()).is_err() {
        tracing::debug!("nobody is waiting for the shutdown to finish");
    }
}

async fn wait_for_local_save(saved: Option<oneshot::Receiver<()>>) {
    let Some(saved) = saved else {
        return;
    };
    if saved.await.is_err() {
        tracing::warn!("the queue save was abandoned during shutdown");
    }
}

async fn flush_to_server(flush: Option<ServerFlush>) {
    if let Some(ServerFlush {
        uploader,
        server,
        state,
    }) = flush
    {
        uploader.flush(server, state).await;
    }
}

impl Controller {
    pub(in crate::controller) fn shut_down(&mut self, ui: UiSettings, done: oneshot::Sender<()>) {
        self.settings.ui = UiSettings {
            follow_current_song: self.settings.ui.follow_current_song,
            ..ui
        };
        self.save_settings();
        self.persist_credential();
        let local_save = self.save_changes_now();
        let server_flush = self.server_flush();
        tokio::spawn(async move {
            tokio::join!(
                wait_for_local_save(local_save),
                flush_to_server(server_flush)
            );
            signal_shutdown_finished(done);
        });
    }

    fn server_flush(&mut self) -> Option<ServerFlush> {
        if self.saved_queue_is_still_loading() {
            return None;
        }
        let (server, state) = self.play_queue_state()?;
        Some(ServerFlush {
            uploader: self.queue_sync.uploader(),
            server,
            state,
        })
    }
}
