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
    time_position: Option<f64>,
    duration: Option<f64>,
    last_position_emitted_at: Option<Instant>,
    position_update_forced: bool,
}

impl PlaybackTracker {
    pub(super) fn new() -> Self {
        Self {
            playing_entry: None,
            loaded: false,
            paused: false,
            idle: true,
            time_position: None,
            duration: None,
            last_position_emitted_at: None,
            position_update_forced: false,
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
        if let Some(position) = self.time_position.filter(|_| is_due) {
            self.emit_position(position, pending_events);
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
        self.time_position = None;
        self.duration = None;
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
            (ObservedProperty::TimePosition, PropertyValue::Double(position)) => {
                self.time_position_changed(position, pending_events);
            }
            (ObservedProperty::TimePosition, _) => self.time_position = None,
            (ObservedProperty::Duration, PropertyValue::Double(duration)) => {
                self.duration = Some(duration);
            }
            (ObservedProperty::Duration, _) => self.duration = None,
            (ObservedProperty::Pause | ObservedProperty::IdleActive, _) => {}
        }
    }

    fn pause_changed(&mut self, paused: bool, pending_events: &mut Vec<PlayerEvent>) {
        if self.paused != paused {
            pending_events.push(pause_transition_event(paused));
        }
        self.paused = paused;
    }

    fn time_position_changed(&mut self, position: f64, pending_events: &mut Vec<PlayerEvent>) {
        self.time_position = Some(position);
        if self.position_update_forced && self.loaded {
            self.emit_position(position, pending_events);
        }
    }

    fn enter_idle(&mut self, pending_events: &mut Vec<PlayerEvent>) {
        if !self.idle {
            self.idle = true;
            pending_events.push(PlayerEvent::Idle);
        }
    }

    fn is_position_ticking(&self) -> bool {
        self.loaded && !self.paused && self.time_position.is_some()
    }

    fn emit_position(&mut self, position: f64, pending_events: &mut Vec<PlayerEvent>) {
        self.position_update_forced = false;
        self.last_position_emitted_at = Some(Instant::now());
        pending_events.push(PlayerEvent::Position {
            position_ms: seconds_to_milliseconds(position),
            duration_ms: self.duration.map(seconds_to_milliseconds),
        });
    }
}

fn pause_transition_event(paused: bool) -> PlayerEvent {
    match paused {
        true => PlayerEvent::Paused,
        false => PlayerEvent::Resumed,
    }
}
