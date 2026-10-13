use feishin_api::{ApiError, Result};
use feishin_core::model::{ServerPlayQueue, Song};

use super::SubsonicServer;
use crate::client::Params;
use crate::extensions::PlayQueueMode;
use crate::types::{GetPlayQueue, GetPlayQueueByIndex, PlayQueue};

fn song_id_at_index(songs: &[Song], current_index: Option<f64>) -> Option<String> {
    current_index
        .filter(|index| *index >= 0.0)
        .and_then(|index| songs.get(index as usize))
        .map(|song| song.id.clone())
}

fn position_milliseconds(position: Option<f64>) -> u64 {
    position
        .filter(|milliseconds| milliseconds.is_finite() && *milliseconds > 0.0)
        .map_or(0, |milliseconds| milliseconds as u64)
}

impl SubsonicServer {
    pub(super) async fn store_play_queue(
        &self,
        song_ids: &[String],
        current_song_id: Option<&str>,
        position_ms: u64,
    ) -> Result<()> {
        let current_index =
            current_song_id.and_then(|id| song_ids.iter().position(|song_id| song_id == id));
        let params = Params::new()
            .many("id", song_ids)
            .add("position", position_ms);
        match self.extensions().await?.play_queue_mode() {
            PlayQueueMode::IndexBased => {
                let params = params.opt("currentIndex", current_index);
                self.send("savePlayQueueByIndex.view", params).await
            }
            PlayQueueMode::CurrentSongId => {
                let current = current_index.and_then(|index| song_ids.get(index));
                self.send("savePlayQueue.view", params.opt("current", current))
                    .await
            }
        }
    }

    pub(super) async fn fetch_play_queue(&self) -> Result<Option<ServerPlayQueue>> {
        let mode = self.extensions().await?.play_queue_mode();
        match self.fetch_raw_play_queue(mode).await {
            Ok(queue) => Ok(queue.map(|queue| self.play_queue_from(queue, mode))),
            Err(ApiError::NotFound) => Ok(None),
            Err(error) => Err(error),
        }
    }

    async fn fetch_raw_play_queue(&self, mode: PlayQueueMode) -> Result<Option<PlayQueue>> {
        match mode {
            PlayQueueMode::IndexBased => self
                .fetch::<GetPlayQueueByIndex>("getPlayQueueByIndex.view", Params::new())
                .await
                .map(|response| response.play_queue_by_index),
            PlayQueueMode::CurrentSongId => self
                .fetch::<GetPlayQueue>("getPlayQueue.view", Params::new())
                .await
                .map(|response| response.play_queue),
        }
    }

    fn play_queue_from(&self, queue: PlayQueue, mode: PlayQueueMode) -> ServerPlayQueue {
        let songs = self.normalize_songs(queue.entry.unwrap_or_default());
        let current_song_id = match mode {
            PlayQueueMode::IndexBased => song_id_at_index(&songs, queue.current_index),
            PlayQueueMode::CurrentSongId => queue.current,
        };
        ServerPlayQueue {
            songs,
            current_song_id,
            position_ms: position_milliseconds(queue.position),
        }
    }
}
