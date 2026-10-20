use feishin_core::queue::{Advance, Uid, YouTubeVideo};
use feishin_youtube::{StreamUrl, YouTubeError};

use super::ResolvePurpose;
use crate::audio::Track;
use crate::controller::{Controller, UiUpdate};

const MAX_CONSECUTIVE_FAILURES: usize = 5;

fn failure_message(title: &str, error: &YouTubeError) -> String {
    match error {
        YouTubeError::NotInstalled => tr!("Install yt-dlp to play YouTube tracks"),
        other => tr!("Could not play \"{}\" from YouTube: {}", title, other),
    }
}

impl Controller {
    pub(in crate::controller) fn resolve_for_playback(
        &mut self,
        uid: Uid,
        video: &YouTubeVideo,
        start_ms: u64,
    ) {
        self.halt_for_resolution(start_ms);
        let request = self.youtube.next_request();
        let purpose = ResolvePurpose::Load { request };
        let task = self.spawn_resolution(uid, &video.video_id, purpose);
        self.youtube.track_load(uid, request, start_ms, task);
        self.update_media_status();
        self.announce_current();
    }

    fn halt_for_resolution(&mut self, start_ms: u64) {
        if (self.loaded.is_some() || self.loading.is_some())
            && let Err(error) = self.deps.audio.stop()
        {
            tracing::warn!("stopping before a YouTube track failed: {error:#}");
        }
        self.loaded = None;
        self.loading = None;
        self.preloaded = None;
        self.is_playing = false;
        self.scrobble_tracker.stop();
        self.position_ms = start_ms;
        self.duration_ms = self.current_duration_ms();
    }

    pub(super) fn finish_load_resolution(
        &mut self,
        uid: Uid,
        request: u64,
        result: Result<StreamUrl, YouTubeError>,
    ) {
        let still_current = self.queue.current().map(|item| item.uid) == Some(uid);
        let Some(start_ms) = self.youtube.finish_load(uid, request) else {
            return;
        };
        if !still_current {
            return;
        }
        match result {
            Ok(stream) => {
                let track = Track {
                    uid,
                    url: stream.url,
                    duration_ms: self.current_duration_ms(),
                };
                self.play_track(&track, start_ms);
            }
            Err(error) => self.skip_unplayable(uid, &error),
        }
    }

    fn skip_unplayable(&mut self, uid: Uid, error: &YouTubeError) {
        let title = self
            .current_track()
            .map(|track| track.title.to_string())
            .unwrap_or_default();
        tracing::warn!("resolving the YouTube track {title} failed: {error}");
        self.deps
            .ui
            .send(UiUpdate::Error(failure_message(&title, error)));
        let failures = self.youtube.count_failure();
        if failures >= MAX_CONSECUTIVE_FAILURES.min(self.queue.len()) {
            self.stop_after_failures();
            return;
        }
        match self.queue.next(Advance::Manual).map(|item| item.uid) {
            Some(next_uid) if next_uid != uid => self.load_current(0),
            _ => self.stop_after_failures(),
        }
    }

    fn stop_after_failures(&mut self) {
        self.youtube.reset_failures();
        self.stop();
        self.publish_now_playing();
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn missing_yt_dlp_asks_for_an_install() {
        assert_eq!(
            failure_message("Song", &YouTubeError::NotInstalled),
            "Install yt-dlp to play YouTube tracks"
        );
        assert_eq!(
            failure_message("Song", &YouTubeError::TimedOut(Duration::from_secs(30))),
            "Could not play \"Song\" from YouTube: yt-dlp did not answer within 30 seconds"
        );
    }
}
