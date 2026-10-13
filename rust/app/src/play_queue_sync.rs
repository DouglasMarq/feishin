//! Server play queue sync, like Feishin's `savePlayQueue` / `getPlayQueue`: saved after changes and on
//! exit, restored on startup when the local queue is empty.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use feishin_api::{ApiError, MusicServer};
use feishin_core::queue::Queue;

#[cfg(not(test))]
const DEBOUNCE: Duration = Duration::from_secs(5);
#[cfg(test)]
const DEBOUNCE: Duration = Duration::ZERO;
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayQueueState {
    pub song_ids: Vec<String>,
    pub current: Option<String>,
    pub position_ms: u64,
}

impl PlayQueueState {
    /// The queue in play order, or `None` when it holds songs from another server.
    pub fn of(queue: &Queue, position_ms: u64, server_id: &str) -> Option<Self> {
        let items = queue.display_order();
        if items.iter().any(|item| item.song.server_id != server_id) {
            return None;
        }
        Some(Self {
            song_ids: items.iter().map(|item| item.song.id.clone()).collect(),
            current: queue.current().map(|item| item.song.id.clone()),
            position_ms: queue.current().map_or(0, |_| position_ms),
        })
    }
}

/// Debounces saves: only the latest scheduled state is sent, and only when it differs from the last one.
#[derive(Clone, Default)]
pub struct PlayQueueSync {
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
    /// Saves `state` after the debounce delay unless another schedule supersedes it. Needs a Tokio runtime.
    pub fn schedule(&self, server: Arc<dyn MusicServer>, state: PlayQueueState) {
        let Ok(mut shared) = self.shared.lock() else {
            return;
        };
        shared.generation += 1;
        let generation = shared.generation;
        drop(shared);
        let shared = Arc::clone(&self.shared);
        tokio::spawn(async move {
            if !DEBOUNCE.is_zero() {
                tokio::time::sleep(DEBOUNCE).await;
            }
            let due = shared
                .lock()
                .ok()
                .and_then(|mut shared| shared.take_if_current(generation, state));
            if let Some(state) = due {
                save(server.as_ref(), &state).await;
            }
        });
    }

    /// Cancels pending saves and saves `state` now, giving up after a short timeout.
    pub async fn flush(&self, server: Arc<dyn MusicServer>, state: PlayQueueState) {
        if let Ok(mut shared) = self.shared.lock() {
            shared.generation += 1;
            shared.saved = Some(state.clone());
        }
        if tokio::time::timeout(SHUTDOWN_TIMEOUT, save(server.as_ref(), &state))
            .await
            .is_err()
        {
            tracing::warn!("saving the play queue to the server timed out");
        }
    }

    /// Marks `state` as already on the server (just restored from it).
    pub fn mark_saved(&self, state: PlayQueueState) {
        if let Ok(mut shared) = self.shared.lock() {
            shared.saved = Some(state);
        }
    }
}

async fn save(server: &dyn MusicServer, state: &PlayQueueState) {
    match server
        .save_play_queue(&state.song_ids, state.current.as_deref(), state.position_ms)
        .await
    {
        Ok(()) => {}
        Err(ApiError::Unsupported(what)) => tracing::debug!("play queue sync: {what}"),
        Err(e) => tracing::warn!("saving the play queue failed: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::test_song;

    fn state(ids: &[&str], position_ms: u64) -> PlayQueueState {
        PlayQueueState {
            song_ids: ids.iter().map(|id| (*id).to_owned()).collect(),
            current: ids.first().map(|id| (*id).to_owned()),
            position_ms,
        }
    }

    #[test]
    fn only_the_latest_changed_state_is_saved() {
        let mut shared = Shared {
            generation: 2,
            saved: None,
        };
        assert_eq!(shared.take_if_current(1, state(&["a"], 0)), None);
        assert_eq!(
            shared.take_if_current(2, state(&["a"], 0)),
            Some(state(&["a"], 0))
        );
        assert_eq!(shared.take_if_current(2, state(&["a"], 0)), None);
        assert!(shared.take_if_current(2, state(&["a"], 5)).is_some());
    }

    #[test]
    fn state_follows_play_order_and_rejects_foreign_songs() {
        let mut queue = Queue::with_seed(1);
        let songs: Vec<_> = (0..3).map(|i| test_song(&format!("s{i}"), 1000)).collect();
        queue.play_now(songs, Some("s1"));
        assert_eq!(
            PlayQueueState::of(&queue, 700, "demo"),
            Some(PlayQueueState {
                song_ids: vec!["s0".into(), "s1".into(), "s2".into()],
                current: Some("s1".into()),
                position_ms: 700,
            })
        );
        assert_eq!(PlayQueueState::of(&queue, 0, "other"), None);
        assert_eq!(
            PlayQueueState::of(&Queue::with_seed(1), 900, "demo"),
            Some(PlayQueueState {
                song_ids: Vec::new(),
                current: None,
                position_ms: 0,
            })
        );
    }
}
