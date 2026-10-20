use std::time::{Duration, Instant};

use super::track_end::track_end_event;
use crate::PlayerEvent;
use crate::ffi::{EndReason, PropertyValue, RawEvent};
use crate::observed_property::ObservedProperty;
use crate::shared::{PlaylistEntryId, Shared};
use crate::time::seconds_to_milliseconds;

const POSITION_INTERVAL: Duration = Duration::from_millis(250);
const IDLE_WAIT_SECONDS: f64 = 1.0;

pub(super) struct PlaybackTracker {
    playing_entry: Option<PlaylistEntryId>,
    loaded: bool,
    paused: bool,
    idle: bool,
    time_position_seconds: Option<f64>,
    duration_seconds: Option<f64>,
    last_position_emitted_at: Option<Instant>,
    position_update_forced: bool,
    stream_title: Option<String>,
}

impl PlaybackTracker {
    pub(super) fn new() -> Self {
        Self {
            playing_entry: None,
            loaded: false,
            paused: false,
            idle: true,
            time_position_seconds: None,
            duration_seconds: None,
            last_position_emitted_at: None,
            position_update_forced: false,
            stream_title: None,
        }
    }

    pub(super) fn handle_raw_event(
        &mut self,
        shared: &Shared,
        event: RawEvent,
        pending_events: &mut Vec<PlayerEvent>,
    ) {
        match event {
            RawEvent::StartFile { entry_id } => self.file_started(shared, entry_id),
            RawEvent::FileLoaded => self.file_loaded(shared, pending_events),
            RawEvent::EndFile { entry_id, reason } => {
                self.file_ended(shared, entry_id, reason, pending_events);
            }
            RawEvent::Seek | RawEvent::PlaybackRestart => self.position_update_forced = true,
            RawEvent::Idle => self.enter_idle(pending_events),
            RawEvent::Property { reply_id, value } => {
                self.property_changed(reply_id, value, pending_events);
            }
            RawEvent::Shutdown | RawEvent::Other => {}
        }
    }

    pub(super) fn wait_timeout_seconds(&self) -> f64 {
        match self.last_position_emitted_at {
            Some(last_emitted_at) if self.is_position_ticking() => POSITION_INTERVAL
                .saturating_sub(last_emitted_at.elapsed())
                .as_secs_f64(),
            _ if self.is_position_ticking() => 0.0,
            _ => IDLE_WAIT_SECONDS,
        }
    }

    pub(super) fn emit_position_when_due(&mut self, pending_events: &mut Vec<PlayerEvent>) {
        if !self.is_position_ticking() {
            return;
        }
        let is_due = self
            .last_position_emitted_at
            .is_none_or(|last_emitted_at| last_emitted_at.elapsed() >= POSITION_INTERVAL);
        if let Some(position_seconds) = self.time_position_seconds.filter(|_| is_due) {
            self.emit_position(position_seconds, pending_events);
        }
    }

    fn file_started(&mut self, shared: &Shared, entry_id: PlaylistEntryId) {
        self.idle = false;
        self.playing_entry = Some(entry_id);
        self.forget_current_file();
        shared
            .lock_entries()
            .next_entry
            .take_if(|next_entry| *next_entry == entry_id);
    }

    fn file_loaded(&mut self, shared: &Shared, pending_events: &mut Vec<PlayerEvent>) {
        self.loaded = true;
        self.position_update_forced = true;
        let uid = self
            .playing_entry
            .and_then(|entry_id| shared.lock_entries().uids.get(&entry_id).copied());
        pending_events.extend(uid.map(|uid| PlayerEvent::Started { uid }));
    }

    fn file_ended(
        &mut self,
        shared: &Shared,
        entry_id: PlaylistEntryId,
        reason: EndReason,
        pending_events: &mut Vec<PlayerEvent>,
    ) {
        if self.playing_entry == Some(entry_id) {
            self.playing_entry = None;
            self.forget_current_file();
        }
        let uid = shared.lock_entries().uids.remove(&entry_id);
        pending_events.extend(track_end_event(uid, reason));
    }

    fn forget_current_file(&mut self) {
        self.loaded = false;
        self.time_position_seconds = None;
        self.duration_seconds = None;
    }

    fn property_changed(
        &mut self,
        reply_id: u64,
        value: PropertyValue,
        pending_events: &mut Vec<PlayerEvent>,
    ) {
        let Some(property) = ObservedProperty::from_reply_id(reply_id) else {
            return;
        };
        match (property, value) {
            (ObservedProperty::Pause, PropertyValue::Flag(paused)) => {
                self.pause_changed(paused, pending_events);
            }
            (ObservedProperty::IdleActive, PropertyValue::Flag(true)) => {
                self.enter_idle(pending_events);
            }
            (ObservedProperty::IdleActive, PropertyValue::Flag(false)) => self.idle = false,
            (ObservedProperty::TimePosition, PropertyValue::Double(position_seconds)) => {
                self.time_position_changed(position_seconds, pending_events);
            }
            (ObservedProperty::TimePosition, _) => self.time_position_seconds = None,
            (ObservedProperty::Duration, PropertyValue::Double(duration_seconds)) => {
                self.duration_seconds = Some(duration_seconds);
            }
            (ObservedProperty::Duration, _) => self.duration_seconds = None,
            (ObservedProperty::StreamTitle, PropertyValue::Text(title)) => {
                self.stream_title_changed(
                    Some(title).filter(|title| !title.trim().is_empty()),
                    pending_events,
                );
            }
            (ObservedProperty::StreamTitle, _) => self.stream_title_changed(None, pending_events),
            (ObservedProperty::Pause | ObservedProperty::IdleActive, _) => {}
        }
    }

    fn stream_title_changed(
        &mut self,
        title: Option<String>,
        pending_events: &mut Vec<PlayerEvent>,
    ) {
        if self.stream_title == title {
            return;
        }
        self.stream_title.clone_from(&title);
        pending_events.push(PlayerEvent::StreamTitle(title));
    }

    fn pause_changed(&mut self, paused: bool, pending_events: &mut Vec<PlayerEvent>) {
        if self.paused != paused {
            pending_events.push(pause_transition_event(paused));
        }
        self.paused = paused;
    }

    fn time_position_changed(
        &mut self,
        position_seconds: f64,
        pending_events: &mut Vec<PlayerEvent>,
    ) {
        self.time_position_seconds = Some(position_seconds);
        if self.position_update_forced && self.loaded {
            self.emit_position(position_seconds, pending_events);
        }
    }

    fn enter_idle(&mut self, pending_events: &mut Vec<PlayerEvent>) {
        if !self.idle {
            self.idle = true;
            pending_events.push(PlayerEvent::Idle);
        }
    }

    fn is_position_ticking(&self) -> bool {
        self.loaded && !self.paused && self.time_position_seconds.is_some()
    }

    fn emit_position(&mut self, position_seconds: f64, pending_events: &mut Vec<PlayerEvent>) {
        self.position_update_forced = false;
        self.last_position_emitted_at = Some(Instant::now());
        pending_events.push(PlayerEvent::Position {
            position_ms: seconds_to_milliseconds(position_seconds),
            duration_ms: self.duration_seconds.map(seconds_to_milliseconds),
        });
    }
}

fn pause_transition_event(paused: bool) -> PlayerEvent {
    match paused {
        true => PlayerEvent::Paused,
        false => PlayerEvent::Resumed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn title_change(tracker: &mut PlaybackTracker, value: PropertyValue) -> Vec<PlayerEvent> {
        let mut pending_events = Vec::new();
        tracker.property_changed(
            ObservedProperty::StreamTitle.reply_id(),
            value,
            &mut pending_events,
        );
        pending_events
    }

    #[test]
    fn stream_titles_are_reported_once_per_change() {
        let mut tracker = PlaybackTracker::new();
        let song = PropertyValue::Text("Artist - Song".into());

        assert_eq!(
            title_change(&mut tracker, song.clone()),
            [PlayerEvent::StreamTitle(Some("Artist - Song".into()))]
        );
        assert_eq!(title_change(&mut tracker, song), []);
        assert_eq!(
            title_change(&mut tracker, PropertyValue::Unavailable),
            [PlayerEvent::StreamTitle(None)]
        );
        assert_eq!(
            title_change(&mut tracker, PropertyValue::Text("  ".into())),
            []
        );
    }
}
