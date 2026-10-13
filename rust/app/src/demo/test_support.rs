use feishin_api::Credential;
use feishin_core::model::{ServerPlayQueue, Song};

use super::scrobbles::ScrobbleCall;
use super::server::FakeServer;
use super::songs::blank_song;

pub fn test_song(id: &str, duration_ms: u64) -> Song {
    blank_song(id, "demo", duration_ms)
}

impl FakeServer {
    pub fn new(song_count: usize) -> Self {
        Self::with_config(Self::config_for_demo(), song_count)
    }

    pub fn saved_play_queue(&self) -> (usize, Option<ServerPlayQueue>) {
        let saves = self
            .queue_saves
            .lock()
            .map(|saves| *saves)
            .unwrap_or_default();
        (saves, self.stored_play_queue())
    }

    pub fn set_credential(&self, credential: Credential) {
        if let Ok(mut current) = self.credential.lock() {
            *current = Some(credential);
        }
    }

    pub fn scrobbles(&self) -> Vec<ScrobbleCall> {
        self.scrobbles
            .lock()
            .map(|calls| calls.clone())
            .unwrap_or_default()
    }
}
