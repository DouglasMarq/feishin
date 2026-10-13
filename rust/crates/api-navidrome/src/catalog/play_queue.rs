use feishin_api::{ApiError, Result};
use feishin_core::model::{ServerPlayQueue, Song};
use reqwest::Method;
use serde::de::IgnoredAny;

use crate::NavidromeServer;
use crate::features::{Features, OpenSubsonicExtension};
use crate::native::{NativeQueue, SaveQueueRequest};
use crate::query::QueryParams;
use crate::subsonic::{self, PlayQueueBody};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueueCursor {
    ByIndex,
    BySongId,
}

impl QueueCursor {
    fn for_features(features: &Features) -> Self {
        if features.has_extension(OpenSubsonicExtension::IndexBasedQueue) {
            return Self::ByIndex;
        }
        Self::BySongId
    }

    const fn save_endpoint(self) -> &'static str {
        match self {
            Self::ByIndex => "savePlayQueueByIndex",
            Self::BySongId => "savePlayQueue",
        }
    }
}

struct QueueSnapshot<'ids> {
    song_ids: &'ids [String],
    current: Option<usize>,
    position_ms: u64,
}

impl NavidromeServer {
    pub(crate) async fn store_play_queue(
        &self,
        song_ids: &[String],
        current_song_id: Option<&str>,
        position_ms: u64,
    ) -> Result<()> {
        let features = self.features().await;
        let snapshot = QueueSnapshot {
            song_ids,
            current: current_song_id
                .and_then(|current| song_ids.iter().position(|id| id == current)),
            position_ms,
        };
        let prefers_native = features.supports_native_play_queue()
            && QueueCursor::for_features(&features) == QueueCursor::BySongId;
        if !prefers_native {
            return self.subsonic_save_queue(&snapshot, &features).await;
        }
        self.native_or_subsonic(
            self.native_save_queue(&snapshot),
            self.subsonic_save_queue(&snapshot, &features),
        )
        .await
    }

    pub(crate) async fn fetch_play_queue(&self) -> Result<Option<ServerPlayQueue>> {
        let features = self.features().await;
        if !features.supports_native_play_queue() {
            return self.subsonic_play_queue(&features).await;
        }
        self.native_or_subsonic(
            self.native_play_queue(),
            self.subsonic_play_queue(&features),
        )
        .await
    }

    async fn native_save_queue(&self, snapshot: &QueueSnapshot<'_>) -> Result<()> {
        let request = SaveQueueRequest {
            ids: snapshot.song_ids,
            position: snapshot.position_ms,
            current: snapshot.current,
        };
        self.native_with_body(Method::POST, "queue", &request)
            .await
            .map(|_| ())
    }

    async fn subsonic_save_queue(
        &self,
        snapshot: &QueueSnapshot<'_>,
        features: &Features,
    ) -> Result<()> {
        let cursor = QueueCursor::for_features(features);
        let mut params: QueryParams = snapshot
            .song_ids
            .iter()
            .map(|id| ("id", id.clone()))
            .collect();
        params.push(("position", snapshot.position_ms.to_string()));
        params.extend(current_entry_param(cursor, snapshot));
        self.subsonic_large::<IgnoredAny>(cursor.save_endpoint(), &params, features)
            .await
            .map(|_| ())
    }

    async fn native_play_queue(&self) -> Result<Option<ServerPlayQueue>> {
        let (queue, _): (NativeQueue, _) = self.native_get("queue", &[]).await?;
        let songs = self.songs_from_native(queue.items.unwrap_or_default());
        if songs.is_empty() {
            return Ok(None);
        }
        let current = queue.current.unwrap_or(0) as usize;
        Ok(Some(ServerPlayQueue {
            current_song_id: song_id_at(&songs, current),
            songs,
            position_ms: queue.position.unwrap_or(0),
        }))
    }

    async fn subsonic_play_queue(&self, features: &Features) -> Result<Option<ServerPlayQueue>> {
        let cursor = QueueCursor::for_features(features);
        let Some(body) = self.fetch_subsonic_queue(cursor).await? else {
            return Ok(None);
        };
        let songs = self.songs_from_subsonic(body.entry.unwrap_or_default());
        if songs.is_empty() {
            return Ok(None);
        }
        let current_song_id = match cursor {
            QueueCursor::ByIndex => body
                .current_index
                .and_then(|index| song_id_at(&songs, index as usize)),
            QueueCursor::BySongId => body.current,
        };
        Ok(Some(ServerPlayQueue {
            songs,
            current_song_id,
            position_ms: body.position.unwrap_or(0),
        }))
    }

    async fn fetch_subsonic_queue(&self, cursor: QueueCursor) -> Result<Option<PlayQueueBody>> {
        if cursor == QueueCursor::ByIndex {
            let response: subsonic::PlayQueueByIndex =
                self.subsonic("getPlayQueueByIndex", &[]).await?;
            return Ok(response.play_queue_by_index);
        }
        match self
            .subsonic::<subsonic::PlayQueue>("getPlayQueue", &[])
            .await
        {
            Ok(response) => Ok(response.play_queue),
            Err(ApiError::NotFound) => Ok(None),
            Err(error) => Err(error),
        }
    }
}

fn song_id_at(songs: &[Song], index: usize) -> Option<String> {
    songs.get(index).map(|song| song.id.clone())
}

fn current_entry_param(
    cursor: QueueCursor,
    snapshot: &QueueSnapshot<'_>,
) -> Option<(&'static str, String)> {
    match cursor {
        QueueCursor::ByIndex => snapshot
            .current
            .map(|index| ("currentIndex", index.to_string())),
        QueueCursor::BySongId => snapshot
            .current
            .and_then(|index| snapshot.song_ids.get(index))
            .map(|id| ("current", id.clone())),
    }
}
