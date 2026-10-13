mod track_end;
mod tracker;

use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::PlayerEvent;
use crate::ffi::{self, RawEvent};
use crate::shared::Shared;
use tracker::PlaybackTracker;

pub(crate) type EventCallback = Box<dyn Fn(PlayerEvent) + Send + Sync>;

pub(crate) fn run(shared: Arc<Shared>, on_event: EventCallback) {
    let handle = shared.mpv.ctx;
    let mut tracker = PlaybackTracker::new();
    let mut pending_events = Vec::new();
    while !shared.stop_requested.load(Ordering::Acquire) {
        match ffi::wait_event_from_event_thread(handle, tracker.wait_timeout_seconds()) {
            Some(RawEvent::Shutdown) => break,
            Some(event) => tracker.handle_raw_event(&shared, event, &mut pending_events),
            None => {}
        }
        tracker.emit_position_when_due(&mut pending_events);
        for event in pending_events.drain(..) {
            on_event(event);
        }
    }
}
