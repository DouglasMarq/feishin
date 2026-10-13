use feishin_core::queue::{Advance, Uid};
use feishin_player::PlayerEvent;

use super::{Controller, UiUpdate};
use crate::controller::scrobbling::now_ms;

enum StartedTrack {
    Loading,
    Preloaded,
    Unrelated,
}

impl Controller {
    pub(super) fn on_player_event(&mut self, event: PlayerEvent) {
        match event {
            PlayerEvent::Started { uid } => self.on_started(uid),
            PlayerEvent::Position {
                position_ms,
                duration_ms,
            } => self.on_position(position_ms, duration_ms),
            PlayerEvent::Paused => self.set_playing(false),
            PlayerEvent::Resumed => self.set_playing(true),
            PlayerEvent::TrackEnded { uid } => self.on_track_ended(uid),
            PlayerEvent::Error { uid, message } => self.on_player_error(uid, message),
            PlayerEvent::Idle => self.on_idle(),
        }
    }

    fn on_position(&mut self, position_ms: u64, duration_ms: Option<u64>) {
        if self.loading.is_some() || self.loaded.is_none() {
            return;
        }
        self.position_ms = position_ms;
        if let Some(duration_ms) = duration_ms.filter(|reported_ms| *reported_ms > 0) {
            self.duration_ms = duration_ms;
        }
        if let Some(submission) = self.scrobble_tracker.position(position_ms, self.playing) {
            self.send_scrobble(submission.song_id, true, Some(submission.timestamp_ms));
        }
        self.publish_playback();
    }

    fn on_track_ended(&mut self, uid: Uid) {
        if self.loaded != Some(uid) || self.preloaded.is_some() {
            return;
        }
        if self.sleep_timer.ends_with_track() {
            self.sleep_at_track_end();
            return;
        }
        match self.queue.next(Advance::Auto).is_some() {
            true => self.load_current(0),
            false => self.stop(),
        }
    }

    fn on_player_error(&mut self, uid: Option<Uid>, message: String) {
        if uid.is_none() || uid == self.loaded {
            self.set_playing(false);
        }
        self.deps.ui.send(UiUpdate::Error(message));
    }

    fn on_idle(&mut self) {
        if self.loading.is_none() && self.loaded.is_some() {
            self.preloaded = None;
            self.set_playing(false);
        }
    }

    fn started_track(&self, uid: Uid) -> StartedTrack {
        match (self.loading == Some(uid), self.preloaded == Some(uid)) {
            (true, _) => StartedTrack::Loading,
            (false, true) => StartedTrack::Preloaded,
            (false, false) => StartedTrack::Unrelated,
        }
    }

    fn on_started(&mut self, uid: Uid) {
        match self.started_track(uid) {
            StartedTrack::Loading => self.loading = None,
            StartedTrack::Preloaded => self.take_over_preloaded(uid),
            StartedTrack::Unrelated => return,
        }
        self.begin_playing_current();
    }

    fn take_over_preloaded(&mut self, uid: Uid) {
        self.preloaded = None;
        if self.queue.next(Advance::Auto).map(|item| item.uid) != Some(uid) {
            self.queue.play_uid(uid);
        }
        self.loaded = Some(uid);
        self.position_ms = 0;
        self.duration_ms = self.current_duration_ms();
        self.publish_now_playing();
        self.publish_queue_current();
        self.schedule_queue_sync();
    }

    fn begin_playing_current(&mut self) {
        self.playing = true;
        if let Some(track) = self.current_track() {
            self.scrobble_tracker
                .start(track.song_id.as_str(), track.duration_ms, now_ms());
            self.send_scrobble(track.song_id.to_string(), false, None);
            self.update_media_metadata(&track);
        }
        self.update_media_status();
        self.publish_playback();
        self.refresh_preload();
    }
}
