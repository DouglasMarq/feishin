use feishin_core::queue::QueueTrack;

use super::Controller;

pub(super) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since_epoch| since_epoch.as_millis() as i64)
        .unwrap_or_default()
}

impl Controller {
    pub(super) fn start_scrobbling(&mut self, track: &QueueTrack) {
        let Some(song_id) = track.server_song_id() else {
            self.scrobble_tracker.stop();
            return;
        };
        self.scrobble_tracker
            .start(song_id, track.duration_ms, now_ms());
        self.send_scrobble(song_id.to_owned(), false, None);
    }

    pub(super) fn send_scrobble(
        &self,
        song_id: String,
        submission: bool,
        timestamp_ms: Option<i64>,
    ) {
        if !self.settings.playback.scrobble {
            return;
        }
        let Some(server) = self.deps.session.get() else {
            return;
        };
        tokio::spawn(async move {
            if let Err(error) = server.scrobble(&song_id, submission, timestamp_ms).await {
                tracing::warn!("scrobble failed: {error}");
            }
        });
    }
}
