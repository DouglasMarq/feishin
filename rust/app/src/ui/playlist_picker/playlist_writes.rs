use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, PoisonError};

use tokio::sync::{Mutex as WriteTurn, OwnedMutexGuard};

static PLAYLIST_WRITES: LazyLock<PlaylistWrites> = LazyLock::new(PlaylistWrites::default);

pub type ExclusiveWrite = OwnedMutexGuard<()>;

#[derive(Default)]
struct PlaylistWrites {
    turns: Mutex<HashMap<String, Arc<WriteTurn<()>>>>,
}

impl PlaylistWrites {
    fn turn_for(&self, playlist_id: &str) -> Arc<WriteTurn<()>> {
        let mut turns = self.turns.lock().unwrap_or_else(PoisonError::into_inner);
        turns.retain(|_, turn| Arc::strong_count(turn) > 1);
        Arc::clone(turns.entry(playlist_id.to_owned()).or_default())
    }
}

pub async fn exclusive_playlist_write(playlist_id: &str) -> ExclusiveWrite {
    PLAYLIST_WRITES.turn_for(playlist_id).lock_owned().await
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn writes_to_one_playlist_wait_for_each_other_but_not_for_other_playlists() {
        let first = exclusive_playlist_write("shared-playlist").await;
        let other = tokio::time::timeout(
            Duration::from_secs(1),
            exclusive_playlist_write("other-playlist"),
        )
        .await;
        assert!(other.is_ok());
        let waiting = tokio::time::timeout(
            Duration::from_millis(20),
            exclusive_playlist_write("shared-playlist"),
        )
        .await;
        assert!(waiting.is_err());
        drop(first);
        let next = tokio::time::timeout(
            Duration::from_secs(1),
            exclusive_playlist_write("shared-playlist"),
        )
        .await;
        assert!(next.is_ok());
    }
}
