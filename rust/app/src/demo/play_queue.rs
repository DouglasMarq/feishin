use feishin_core::model::{ServerPlayQueue, Song};

use super::server::FakeServer;
use super::songs::blank_song;

impl FakeServer {
    pub(super) fn store_play_queue(
        &self,
        song_ids: &[String],
        current_song_id: Option<&str>,
        position_ms: u64,
    ) {
        let songs = song_ids
            .iter()
            .map(|song_id| self.queued_song(song_id))
            .collect();
        match (self.play_queue.lock(), self.queue_saves.lock()) {
            (Ok(mut queue), Ok(mut saves)) => {
                *queue = Some(ServerPlayQueue {
                    songs,
                    current_song_id: current_song_id.map(str::to_owned),
                    position_ms,
                });
                *saves += 1;
            }
            _ => tracing::warn!("demo play queue storage unavailable"),
        }
    }

    pub(super) fn stored_play_queue(&self) -> Option<ServerPlayQueue> {
        self.play_queue.lock().ok().and_then(|queue| queue.clone())
    }

    fn queued_song(&self, song_id: &str) -> Song {
        self.song_index_in_library(song_id).map_or_else(
            || blank_song(song_id, &self.config.id, 0),
            |index| self.song_at(index),
        )
    }
}
