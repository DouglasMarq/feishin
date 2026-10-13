mod song_ids;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use feishin_api::{ApiError, MusicServer};
use feishin_core::queue::Queue;

use song_ids::SongIdCache;

#[cfg(not(test))]
pub const DEBOUNCE: Duration = Duration::from_secs(5);
#[cfg(test)]
pub const DEBOUNCE: Duration = Duration::ZERO;
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayQueueState {
    pub song_ids: Arc<[String]>,
    pub current: Option<String>,
    pub position_ms: u64,
}

#[derive(Default)]
pub struct PlayQueueSync {
    uploader: PlayQueueUploader,
    song_ids: SongIdCache,
}

#[derive(Clone, Default)]
pub struct PlayQueueUploader {
    shared: Arc<Mutex<Shared>>,
}

#[derive(Default)]
struct Shared {
    generation: u64,
    saved: Option<PlayQueueState>,
}

impl Shared {
    fn take_if_current(
        &mut self,
        generation: u64,
        state: PlayQueueState,
    ) -> Option<PlayQueueState> {
        if generation != self.generation || self.saved.as_ref() == Some(&state) {
            return None;
        }
        self.saved = Some(state.clone());
        Some(state)
    }
}

impl PlayQueueSync {
    pub fn next_generation(&self) -> u64 {
        self.uploader.with_shared(|shared| {
            shared.generation += 1;
            shared.generation
        })
    }

    pub fn is_current(&self, generation: u64) -> bool {
        self.uploader
            .with_shared(|shared| shared.generation == generation)
    }

    pub fn state_of(
        &mut self,
        queue: &Queue,
        position_ms: u64,
        server_id: &str,
    ) -> Option<PlayQueueState> {
        let current = queue.current();
        Some(PlayQueueState {
            song_ids: self.song_ids.song_ids(queue, server_id)?,
            current: current.map(|item| item.track.song_id.to_string()),
            position_ms: current.map_or(0, |_| position_ms),
        })
    }

    pub fn forget_song_ids(&mut self) {
        self.song_ids = SongIdCache::default();
    }

    pub fn upload(&self, server: Arc<dyn MusicServer>, generation: u64, state: PlayQueueState) {
        let shared = Arc::clone(&self.uploader.shared);
        tokio::spawn(async move {
            let due = shared
                .lock()
                .ok()
                .and_then(|mut shared| shared.take_if_current(generation, state));
            if let Some(state) = due {
                save_to_server(server.as_ref(), &state).await;
            }
        });
    }

    pub fn uploader(&self) -> PlayQueueUploader {
        self.uploader.clone()
    }

    pub fn mark_saved(&self, state: PlayQueueState) {
        self.uploader
            .with_shared(|shared| shared.saved = Some(state));
    }
}

impl PlayQueueUploader {
    fn with_shared<Output: Default>(&self, action: impl FnOnce(&mut Shared) -> Output) -> Output {
        self.shared
            .lock()
            .map(|mut shared| action(&mut shared))
            .unwrap_or_default()
    }

    pub async fn flush(&self, server: Arc<dyn MusicServer>, state: PlayQueueState) {
        self.with_shared(|shared| {
            shared.generation += 1;
            shared.saved = Some(state.clone());
        });
        if tokio::time::timeout(SHUTDOWN_TIMEOUT, save_to_server(server.as_ref(), &state))
            .await
            .is_err()
        {
            tracing::warn!("saving the play queue to the server timed out");
        }
    }
}

async fn save_to_server(server: &dyn MusicServer, state: &PlayQueueState) {
    match server
        .save_play_queue(&state.song_ids, state.current.as_deref(), state.position_ms)
        .await
    {
        Ok(()) => {}
        Err(ApiError::Unsupported(unsupported)) => {
            tracing::debug!("play queue sync: {unsupported}")
        }
        Err(error) => tracing::warn!("saving the play queue failed: {error}"),
    }
}

#[cfg(test)]
mod tests;
