use feishin_core::model::ServerConfig;

use super::{Controller, UiUpdate};

impl Controller {
    pub(super) fn connected(&mut self, config: ServerConfig, others: Vec<ServerConfig>) {
        if self.is_switching_server(&config.id) {
            self.clear_queue();
        }
        self.remember_servers(config, others);
        self.save_settings();
        self.persist_credential();
        self.resume_queue_after_connect();
    }

    fn is_switching_server(&self, server_id: &str) -> bool {
        self.settings
            .current_server_id
            .as_ref()
            .is_some_and(|current_id| current_id != server_id)
    }

    fn remember_servers(&mut self, config: ServerConfig, others: Vec<ServerConfig>) {
        let current_id = config.id.clone();
        let servers = &mut self.settings.servers;
        for server in others.into_iter().chain(std::iter::once(config)) {
            servers.retain(|saved| saved.id != server.id);
            servers.push(server);
        }
        self.settings.current_server_id = Some(current_id);
    }

    fn resume_queue_after_connect(&mut self) {
        if self.queue.is_empty() {
            self.fetch_server_queue();
            return;
        }
        let Some(track) = self.current_track() else {
            return;
        };
        self.update_media_metadata(&track);
        self.deps
            .ui
            .send(UiUpdate::NowPlaying(Some(Box::new(track))));
    }

    pub(super) fn remove_server(&mut self, server_id: &str) {
        if let Err(error) = self.deps.store.delete_credential(server_id) {
            tracing::warn!("deleting the saved login failed: {error:#}");
        }
        self.settings
            .servers
            .retain(|server| server.id != server_id);
        if self.settings.current_server_id.as_deref() == Some(server_id) {
            self.settings.current_server_id = None;
            self.deps.session.set(None);
            self.clear_queue();
        }
        self.save_settings();
    }
}
