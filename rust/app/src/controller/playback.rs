use std::sync::Arc;

use feishin_api::StreamOptions;
use feishin_core::queue::{Advance, QueueItem, TrackSource, YouTubeVideo};

use super::{Controller, UiUpdate};
use crate::audio::Track;

const PREVIOUS_RESTARTS_AFTER_MS: u64 = 10_000;

pub(super) enum Playable {
    Ready(Track),
    NeedsResolution(Arc<YouTubeVideo>),
    NotConnected,
}

impl Controller {
    #[cfg(test)]
    pub fn play_now(&mut self, songs: Vec<feishin_core::model::Song>, song_id: &str) {
        self.queue.play_now(songs, Some(song_id));
        self.publish_queue();
        self.load_current(0);
    }

    fn server_stream(&self, item: &QueueItem) -> Option<Track> {
        let server = self.deps.session.get()?;
        let playback = &self.settings.playback;
        let options = StreamOptions {
            max_bitrate_kbps: playback.max_bitrate_kbps,
            format: playback.transcode_format.clone(),
        };
        Some(Track {
            uid: item.uid,
            url: server.stream_url(item.track.song_id.as_str(), &options),
            duration_ms: item.track.duration_ms,
        })
    }

    fn youtube_stream(&self, item: &QueueItem, video: &Arc<YouTubeVideo>) -> Playable {
        match self.youtube.cached_url(&video.video_id) {
            Some(url) => Playable::Ready(Track {
                uid: item.uid,
                url,
                duration_ms: item.track.duration_ms,
            }),
            None => Playable::NeedsResolution(Arc::clone(video)),
        }
    }

    pub(super) fn playable(&self, item: &QueueItem) -> Playable {
        match &item.track.source {
            TrackSource::Server => self
                .server_stream(item)
                .map_or(Playable::NotConnected, Playable::Ready),
            TrackSource::YouTube(video) => self.youtube_stream(item, video),
        }
    }

    pub(super) fn load_current(&mut self, start_ms: u64) {
        let current = self
            .queue
            .current()
            .map(|item| (item.uid, self.playable(item)));
        let Some((uid, playable)) = current else {
            self.stop();
            return;
        };
        match playable {
            Playable::Ready(track) => self.play_track(&track, start_ms),
            Playable::NeedsResolution(video) => self.resolve_for_playback(uid, &video, start_ms),
            Playable::NotConnected => self
                .deps
                .ui
                .send(UiUpdate::Error("Not connected to a server".into())),
        }
    }

    pub(super) fn play_track(&mut self, track: &Track, start_ms: u64) {
        self.youtube.cancel_load();
        self.scrobble_tracker.stop();
        self.preloaded = None;
        self.position_ms = start_ms;
        self.duration_ms = self.current_duration_ms();
        self.start_audio(track, start_ms);
        self.announce_current();
    }

    pub(super) fn announce_current(&mut self) {
        self.publish_now_playing();
        self.publish_queue_current();
        self.publish_playback();
        self.schedule_queue_sync();
    }

    fn start_audio(&mut self, track: &Track, start_ms: u64) {
        match self.deps.audio.load(track, start_ms) {
            Ok(()) => {
                self.loaded = Some(track.uid);
                self.loading = Some(track.uid);
                self.playing = true;
            }
            Err(error) => {
                self.loaded = None;
                self.loading = None;
                self.playing = false;
                self.deps
                    .ui
                    .send(UiUpdate::Error(format!("Playback failed: {error:#}")));
            }
        }
    }

    pub(super) fn stop(&mut self) {
        if let Err(error) = self.deps.audio.stop() {
            tracing::warn!("stop failed: {error:#}");
        }
        self.youtube.cancel_load();
        self.loaded = None;
        self.loading = None;
        self.preloaded = None;
        self.playing = false;
        self.position_ms = 0;
        self.scrobble_tracker.stop();
        self.update_media_status();
        self.publish_playback();
    }

    pub(super) fn toggle_pause(&mut self) {
        if self.loaded.is_none() {
            self.resume_from_saved_position();
            return;
        }
        let result = match self.playing {
            true => self.deps.audio.pause(),
            false => self.deps.audio.play(),
        };
        match result {
            Ok(()) => self.set_playing(!self.playing),
            Err(error) => tracing::warn!("toggling playback failed: {error:#}"),
        }
    }

    fn resume_from_saved_position(&mut self) {
        if self.queue.current().is_some() && !self.youtube.is_loading() {
            self.load_current(self.position_ms);
        }
    }

    pub(super) fn set_playing(&mut self, playing: bool) {
        if self.playing == playing {
            return;
        }
        self.playing = playing;
        self.update_media_status();
        self.publish_playback();
        if !playing {
            self.schedule_queue_sync();
        }
    }

    pub(super) fn next(&mut self) {
        if self.queue.next(Advance::Manual).is_some() {
            self.load_current(0);
        }
    }

    pub(super) fn previous(&mut self) {
        if self.position_ms > PREVIOUS_RESTARTS_AFTER_MS {
            self.seek(0);
            return;
        }
        let current = self.queue.current().map(|item| item.uid);
        match self.queue.previous().map(|item| item.uid) {
            Some(uid) if Some(uid) != current => self.load_current(0),
            _ => self.seek(0),
        }
    }

    pub(super) fn seek(&mut self, position_ms: u64) {
        if self.loaded.is_some()
            && let Err(error) = self.deps.audio.seek(position_ms)
        {
            tracing::warn!("seek failed: {error:#}");
            return;
        }
        self.position_ms = position_ms;
        self.youtube.move_load_start(position_ms);
        self.update_media_status();
        self.publish_playback();
        self.schedule_queue_sync();
    }
}
