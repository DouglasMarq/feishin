use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use feishin_core::queue::Uid;

use crate::ffi::{self, EndReason, PropValue, RawEvent};
use crate::{PlayerEvent, Shared};

pub(crate) const PROP_PAUSE: u64 = 1;
pub(crate) const PROP_TIME_POS: u64 = 2;
pub(crate) const PROP_DURATION: u64 = 3;
pub(crate) const PROP_IDLE: u64 = 4;

const POSITION_INTERVAL: Duration = Duration::from_millis(250);
const IDLE_WAIT_SECS: f64 = 1.0;

pub(crate) type Callback = Box<dyn Fn(PlayerEvent) + Send + Sync>;

struct Tracker {
    playing_entry: Option<i64>,
    loaded: bool,
    paused: bool,
    idle: bool,
    time_pos: Option<f64>,
    duration: Option<f64>,
    last_position: Option<Instant>,
    force_position: bool,
}

pub(crate) fn run(shared: Arc<Shared>, on_event: Callback) {
    let ctx = shared.mpv.ctx;
    let mut tracker = Tracker {
        playing_entry: None,
        loaded: false,
        paused: false,
        idle: true,
        time_pos: None,
        duration: None,
        last_position: None,
        force_position: false,
    };
    let mut out = Vec::new();
    while !shared.stop.load(Ordering::Acquire) {
        match ffi::wait_event(ctx, tracker.timeout()) {
            Some(RawEvent::Shutdown) => break,
            Some(event) => tracker.handle(&shared, event, &mut out),
            None => {}
        }
        tracker.tick(&mut out);
        for event in out.drain(..) {
            on_event(event);
        }
    }
}

impl Tracker {
    fn handle(&mut self, shared: &Shared, event: RawEvent, out: &mut Vec<PlayerEvent>) {
        match event {
            RawEvent::StartFile { entry_id } => {
                self.idle = false;
                self.playing_entry = Some(entry_id);
                self.loaded = false;
                self.time_pos = None;
                self.duration = None;
                let mut state = shared.lock();
                if state.next_entry == Some(entry_id) {
                    state.next_entry = None;
                }
            }
            RawEvent::FileLoaded => {
                self.loaded = true;
                self.force_position = true;
                let uid = self
                    .playing_entry
                    .and_then(|id| shared.lock().uids.get(&id).copied());
                if let Some(uid) = uid {
                    out.push(PlayerEvent::Started { uid });
                }
            }
            RawEvent::EndFile { entry_id, reason } => {
                if self.playing_entry == Some(entry_id) {
                    self.playing_entry = None;
                    self.loaded = false;
                    self.time_pos = None;
                    self.duration = None;
                }
                let uid = shared.lock().uids.remove(&entry_id);
                if let Some(event) = end_event(uid, reason) {
                    out.push(event);
                }
            }
            RawEvent::Seek | RawEvent::PlaybackRestart => self.force_position = true,
            RawEvent::Idle => self.enter_idle(out),
            RawEvent::Property { id, value } => self.property(id, value, out),
            RawEvent::Shutdown | RawEvent::Other => {}
        }
    }

    fn property(&mut self, id: u64, value: PropValue, out: &mut Vec<PlayerEvent>) {
        match (id, value) {
            (PROP_PAUSE, PropValue::Flag(paused)) => {
                if self.paused != paused {
                    out.push(if paused {
                        PlayerEvent::Paused
                    } else {
                        PlayerEvent::Resumed
                    });
                }
                self.paused = paused;
            }
            (PROP_IDLE, PropValue::Flag(true)) => self.enter_idle(out),
            (PROP_IDLE, PropValue::Flag(false)) => self.idle = false,
            (PROP_TIME_POS, PropValue::Double(pos)) => {
                self.time_pos = Some(pos);
                if self.force_position && self.loaded {
                    self.emit_position(pos, out);
                }
            }
            (PROP_TIME_POS, _) => self.time_pos = None,
            (PROP_DURATION, PropValue::Double(duration)) => self.duration = Some(duration),
            (PROP_DURATION, _) => self.duration = None,
            _ => {}
        }
    }

    fn enter_idle(&mut self, out: &mut Vec<PlayerEvent>) {
        if !self.idle {
            self.idle = true;
            out.push(PlayerEvent::Idle);
        }
    }

    fn ticking(&self) -> bool {
        self.loaded && !self.paused && self.time_pos.is_some()
    }

    fn timeout(&self) -> f64 {
        match self.last_position {
            Some(last) if self.ticking() => POSITION_INTERVAL
                .saturating_sub(last.elapsed())
                .as_secs_f64(),
            _ if self.ticking() => 0.0,
            _ => IDLE_WAIT_SECS,
        }
    }

    fn tick(&mut self, out: &mut Vec<PlayerEvent>) {
        if !self.ticking() {
            return;
        }
        let due = self
            .last_position
            .is_none_or(|last| last.elapsed() >= POSITION_INTERVAL);
        if let (true, Some(pos)) = (due, self.time_pos) {
            self.emit_position(pos, out);
        }
    }

    fn emit_position(&mut self, pos: f64, out: &mut Vec<PlayerEvent>) {
        self.force_position = false;
        self.last_position = Some(Instant::now());
        out.push(PlayerEvent::Position {
            position_ms: secs_to_ms(pos),
            duration_ms: self.duration.map(secs_to_ms),
        });
    }
}

fn end_event(uid: Option<Uid>, reason: EndReason) -> Option<PlayerEvent> {
    match reason {
        EndReason::Eof => uid.map(|uid| PlayerEvent::TrackEnded { uid }),
        EndReason::Error(code) => uid.map(|uid| PlayerEvent::Error {
            uid: Some(uid),
            message: ffi::error_string(code),
        }),
        EndReason::Stop | EndReason::Quit | EndReason::Redirect | EndReason::Unknown => None,
    }
}

pub(crate) fn secs_to_ms(secs: f64) -> u64 {
    if secs.is_finite() && secs > 0.0 {
        (secs * 1000.0).round() as u64
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_seconds_to_milliseconds() {
        assert_eq!(secs_to_ms(1.2345), 1235);
        assert_eq!(secs_to_ms(0.0), 0);
        assert_eq!(secs_to_ms(-3.0), 0);
        assert_eq!(secs_to_ms(f64::NAN), 0);
        assert_eq!(secs_to_ms(f64::INFINITY), 0);
    }

    #[test]
    fn only_natural_end_and_errors_produce_events() {
        assert_eq!(
            end_event(Some(4), EndReason::Eof),
            Some(PlayerEvent::TrackEnded { uid: 4 })
        );
        assert!(matches!(
            end_event(Some(4), EndReason::Error(-13)),
            Some(PlayerEvent::Error { uid: Some(4), .. })
        ));
        for reason in [EndReason::Stop, EndReason::Quit, EndReason::Redirect] {
            assert_eq!(end_event(Some(4), reason), None);
        }
        assert_eq!(end_event(None, EndReason::Eof), None);
    }
}
