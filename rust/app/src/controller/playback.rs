use std::sync::Arc;

use feishin_api::StreamOptions;
use feishin_core::model::{Song, SongListQuery};
use feishin_core::queue::{Advance, QueueItem, QueueTrack};

use super::{Command, Controller, UiUpdate, forward_command};
use crate::audio::Track;
use crate::library::fetch_all_songs;

const PREVIOUS_RESTARTS_AFTER_MS: u64 = 10_000;

impl Controller {
    pub(super) fn play_from_list(&mut self, query: SongListQuery, song_id: String) {
        self.play_request += 1;
        let request = self.play_request;
        let Some(server) = self.deps.session.get() else {
            return;
        };
        let commands = self.deps.commands.clone();
        let ui = Arc::clone(&self.deps.ui);
        tokio::spawn(async move {
            match fetch_all_songs(server.as_ref(), &query).await {
                Ok(songs) => forward_command(
                    &commands,
                    Command::PlayNow {
                        request,
                        songs,
                        song_id,
                    },
                ),
                Err(error) => ui.send(UiUpdate::Error(format!("Could not load the list: {error}"))),
            }
        });
    }

    pub(super) fn play_requested(&mut self, request: u64, songs: Vec<Song>, song_id: &str) {
        if request != self.play_request {
            return;
        }
        self.play_now(songs, song_id);
    }

    pub fn play_now(&mut self, songs: Vec<Song>, song_id: &str) {
        self.queue.play_now(songs, Some(song_id));
        self.publish_queue();
        self.load_current(0);
    }

    pub(super) fn stream_track(&self, item: &QueueItem) -> Option<Track> {
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

    pub(super) fn load_current(&mut self, start_ms: u64) {
        let Some(item) = self.queue.current() else {
            self.stop();
            return;
        };
        let Some(track) = self.stream_track(item) else {
            self.deps
                .ui
                .send(UiUpdate::Error("Not connected to a server".into()));
            return;
        };
        let now_playing = QueueTrack::clone(&item.track);
        self.scrobble_tracker.stop();
        self.preloaded = None;
        self.position_ms = start_ms;
        self.duration_ms = now_playing.duration_ms;
        self.start_audio(&track, start_ms);
        self.deps
            .ui
            .send(UiUpdate::NowPlaying(Some(Box::new(now_playing))));
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
        if self.queue.current().is_some() {
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
        self.update_media_status();
        self.publish_playback();
        self.schedule_queue_sync();
    }
}
