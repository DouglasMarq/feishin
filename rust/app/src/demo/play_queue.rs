use feishin_core::model::{ServerPlayQueue, Song};

use super::server::FakeServer;
use super::songs::blank_song;

pub(super) struct DemoPlayQueue {
    song_ids: Vec<String>,
    current_song_id: Option<String>,
    position_ms: u64,
}

impl FakeServer {
    pub(super) fn store_play_queue(
        &self,
        song_ids: &[String],
        current_song_id: Option<&str>,
        position_ms: u64,
    ) {
        let stored = DemoPlayQueue {
            song_ids: song_ids.to_vec(),
            current_song_id: current_song_id.map(str::to_owned),
            position_ms,
        };
        match (self.play_queue.lock(), self.queue_saves.lock()) {
            (Ok(mut queue), Ok(mut saves)) => {
                *queue = Some(stored);
                *saves += 1;
            }
            _ => tracing::warn!("demo play queue storage unavailable"),
        }
    }

    pub(super) fn stored_play_queue(&self) -> Option<ServerPlayQueue> {
        let queue = self.play_queue.lock().ok()?;
        let stored = queue.as_ref()?;
        Some(ServerPlayQueue {
            songs: stored
                .song_ids
                .iter()
                .map(|song_id| self.queued_song(song_id))
                .collect(),
            current_song_id: stored.current_song_id.clone(),
            position_ms: stored.position_ms,
        })
    }

    fn queued_song(&self, song_id: &str) -> Song {
        self.song_index_in_library(song_id).map_or_else(
            || blank_song(song_id, &self.config.id, 0),
            |index| self.song_at(index),
        )
    }
}
