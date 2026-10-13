use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use feishin_player::{Player, PlayerConfig, PlayerEvent};

const WAIT_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) fn sine_wave_url(seconds: u32) -> String {
    format!("av://lavfi:sine=frequency=440:duration={seconds}")
}

#[derive(Default)]
pub(crate) struct Recorder {
    events: Mutex<Vec<PlayerEvent>>,
    changed: Condvar,
}

impl Recorder {
    pub(crate) fn push(&self, event: PlayerEvent) {
        self.events.lock().unwrap().push(event);
        self.changed.notify_all();
    }

    pub(crate) fn snapshot(&self) -> Vec<PlayerEvent> {
        self.events.lock().unwrap().clone()
    }

    pub(crate) fn wait_for(
        &self,
        description: &str,
        predicate: impl Fn(&PlayerEvent) -> bool,
    ) -> PlayerEvent {
        let deadline = Instant::now() + WAIT_TIMEOUT;
        let mut events = self.events.lock().unwrap();
        let mut checked_count = 0;
        loop {
            if let Some(found) = events[checked_count..]
                .iter()
                .find(|event| predicate(event))
            {
                return found.clone();
            }
            checked_count = events.len();
            let now = Instant::now();
            assert!(
                now < deadline,
                "timed out waiting for {description}: {events:?}"
            );
            events = self.changed.wait_timeout(events, deadline - now).unwrap().0;
        }
    }

    pub(crate) fn wait_for_event(&self, expected: PlayerEvent) {
        self.wait_for(&format!("{expected:?}"), |event| *event == expected);
    }

    pub(crate) fn wait_after(
        &self,
        event_count_before: usize,
        description: &str,
        predicate: impl Fn(&PlayerEvent) -> bool,
    ) {
        let deadline = Instant::now() + WAIT_TIMEOUT;
        let mut events = self.events.lock().unwrap();
        while !events[event_count_before..].iter().any(&predicate) {
            let now = Instant::now();
            assert!(
                now < deadline,
                "timed out waiting for {description}: {events:?}"
            );
            events = self.changed.wait_timeout(events, deadline - now).unwrap().0;
        }
    }

    pub(crate) fn wait_for_event_after(&self, event_count_before: usize, expected: PlayerEvent) {
        self.wait_after(event_count_before, &format!("{expected:?}"), |event| {
            *event == expected
        });
    }

    pub(crate) fn event_count(&self) -> usize {
        self.events.lock().unwrap().len()
    }

    pub(crate) fn milestones(&self) -> Vec<PlayerEvent> {
        self.snapshot()
            .into_iter()
            .filter(|event| !matches!(event, PlayerEvent::Position { .. }))
            .collect()
    }

    pub(crate) fn positions(&self) -> Vec<(u64, Option<u64>)> {
        self.snapshot()
            .into_iter()
            .filter_map(|event| match event {
                PlayerEvent::Position {
                    position_ms,
                    duration_ms,
                } => Some((position_ms, duration_ms)),
                _ => None,
            })
            .collect()
    }
}

pub(crate) fn player_with(config: PlayerConfig) -> (Player, Arc<Recorder>) {
    let recorder = Arc::new(Recorder::default());
    let event_sink = Arc::clone(&recorder);
    let player =
        Player::with_null_output(config, Box::new(move |event| event_sink.push(event))).unwrap();
    (player, recorder)
}

pub(crate) fn player() -> (Player, Arc<Recorder>) {
    player_with(PlayerConfig::default())
}

pub(crate) fn has_error(events: &[PlayerEvent]) -> bool {
    events
        .iter()
        .any(|event| matches!(event, PlayerEvent::Error { .. }))
}
