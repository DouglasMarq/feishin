use std::sync::Arc;
use std::time::{Duration, Instant};

use feishin_core::queue::{QueueTrack, QueueView, RepeatMode, Uid};

use crate::protocol::{RemoteQueue, StateUpdate};

const POSITION_INTERVAL: Duration = Duration::from_secs(1);
const SEEK_THRESHOLD_MS: u64 = 2_000;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HostState {
    pub now_playing: Option<Arc<QueueTrack>>,
    pub is_playing: bool,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub volume: u8,
    pub is_muted: bool,
    pub is_shuffled: bool,
    pub repeat: RepeatMode,
    pub queue: QueueView,
}

impl HostState {
    fn current_uid(&self) -> Option<Uid> {
        self.queue.current().map(|item| item.uid)
    }
}

pub fn remote_queue(view: &QueueView) -> RemoteQueue {
    RemoteQueue {
        items: (0..view.len())
            .filter_map(|position| view.item_at(position).cloned())
            .collect(),
        current_uid: view.current().map(|item| item.uid),
    }
}

struct Sent {
    state: HostState,
    playback_at: Instant,
}

#[derive(Default)]
pub struct Outbox {
    sent: Option<Sent>,
}

impl Outbox {
    pub fn updates(&mut self, state: &HostState, now: Instant) -> Vec<StateUpdate> {
        let Some(sent) = &self.sent else {
            self.sent = Some(Sent {
                state: state.clone(),
                playback_at: now,
            });
            return full_state(state);
        };
        let mut updates = changed_parts(&sent.state, state);
        let playback_due = has_playback_changed(sent, state, now);
        if playback_due {
            updates.push(playback_of(state));
        }
        let playback_at = match playback_due {
            true => now,
            false => sent.playback_at,
        };
        let shown_position = match playback_due {
            true => state.position_ms,
            false => sent.state.position_ms,
        };
        self.sent = Some(Sent {
            state: HostState {
                position_ms: shown_position,
                ..state.clone()
            },
            playback_at,
        });
        updates
    }
}

fn full_state(state: &HostState) -> Vec<StateUpdate> {
    vec![
        StateUpdate::Volume {
            volume: state.volume,
            is_muted: state.is_muted,
        },
        StateUpdate::Modes {
            is_shuffled: state.is_shuffled,
            repeat: state.repeat,
        },
        StateUpdate::Queue {
            queue: remote_queue(&state.queue),
        },
        now_playing_of(state),
        playback_of(state),
    ]
}

fn changed_parts(sent: &HostState, state: &HostState) -> Vec<StateUpdate> {
    let mut updates = Vec::new();
    if (sent.volume, sent.is_muted) != (state.volume, state.is_muted) {
        updates.push(StateUpdate::Volume {
            volume: state.volume,
            is_muted: state.is_muted,
        });
    }
    if (sent.is_shuffled, sent.repeat) != (state.is_shuffled, state.repeat) {
        updates.push(StateUpdate::Modes {
            is_shuffled: state.is_shuffled,
            repeat: state.repeat,
        });
    }
    match (
        sent.queue.shares_layout_with(&state.queue),
        sent.current_uid() == state.current_uid(),
    ) {
        (false, _) => updates.push(StateUpdate::Queue {
            queue: remote_queue(&state.queue),
        }),
        (true, false) => updates.push(StateUpdate::QueueCurrent {
            current_uid: state.current_uid(),
        }),
        (true, true) => {}
    }
    if sent.now_playing != state.now_playing {
        updates.push(now_playing_of(state));
    }
    updates
}

fn has_playback_changed(sent: &Sent, state: &HostState, now: Instant) -> bool {
    let previous = &sent.state;
    if (previous.is_playing, previous.duration_ms) != (state.is_playing, state.duration_ms) {
        return true;
    }
    let elapsed = now.saturating_duration_since(sent.playback_at);
    let elapsed_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
    let expected = match state.is_playing {
        true => previous.position_ms.saturating_add(elapsed_ms),
        false => previous.position_ms,
    };
    let jumped = expected.abs_diff(state.position_ms) > SEEK_THRESHOLD_MS;
    jumped || (elapsed >= POSITION_INTERVAL && previous.position_ms != state.position_ms)
}

fn now_playing_of(state: &HostState) -> StateUpdate {
    StateUpdate::NowPlaying {
        track: state.now_playing.as_deref().cloned(),
    }
}

fn playback_of(state: &HostState) -> StateUpdate {
    StateUpdate::Playback {
        is_playing: state.is_playing,
        position_ms: state.position_ms,
        duration_ms: state.duration_ms,
    }
}

#[cfg(test)]
mod tests {
    use feishin_core::queue::{Advance, Queue, YouTubeTrack};

    use super::*;

    fn queue_of(count: usize) -> Queue {
        let mut queue = Queue::with_seed(1);
        let tracks: Vec<QueueTrack> = (0..count)
            .map(|index| {
                QueueTrack::from(&YouTubeTrack {
                    video_id: format!("v{index}"),
                    title: format!("Song {index}"),
                    artists: vec!["Artist".to_owned()],
                    album: None,
                    thumbnail_url: None,
                    duration_ms: 200_000,
                })
            })
            .collect();
        queue.play_tracks_now(tracks, Some(0));
        queue
    }

    fn kinds(updates: &[StateUpdate]) -> Vec<&'static str> {
        updates
            .iter()
            .map(|update| match update {
                StateUpdate::NowPlaying { .. } => "now_playing",
                StateUpdate::Playback { .. } => "playback",
                StateUpdate::Volume { .. } => "volume",
                StateUpdate::Modes { .. } => "modes",
                StateUpdate::Queue { .. } => "queue",
                StateUpdate::QueueCurrent { .. } => "queue_current",
            })
            .collect()
    }

    #[test]
    fn the_first_push_is_the_whole_state_and_nothing_repeats() {
        let start = Instant::now();
        let mut outbox = Outbox::default();
        let state = HostState {
            queue: queue_of(3).view(),
            ..HostState::default()
        };
        assert_eq!(
            kinds(&outbox.updates(&state, start)),
            ["volume", "modes", "queue", "now_playing", "playback"]
        );
        assert!(outbox.updates(&state, start).is_empty());
    }

    #[test]
    fn positions_are_sent_once_a_second_but_seeks_and_pauses_at_once() {
        let start = Instant::now();
        let mut outbox = Outbox::default();
        let mut state = HostState {
            is_playing: true,
            duration_ms: 200_000,
            ..HostState::default()
        };
        outbox.updates(&state, start);
        state.position_ms = 250;
        assert!(
            outbox
                .updates(&state, start + Duration::from_millis(250))
                .is_empty()
        );
        state.position_ms = 1_000;
        assert_eq!(
            kinds(&outbox.updates(&state, start + Duration::from_millis(1_000))),
            ["playback"]
        );
        state.position_ms = 90_000;
        assert_eq!(
            kinds(&outbox.updates(&state, start + Duration::from_millis(1_100))),
            ["playback"]
        );
        state.is_playing = false;
        assert_eq!(
            kinds(&outbox.updates(&state, start + Duration::from_millis(1_150))),
            ["playback"]
        );
    }

    #[test]
    fn queue_edits_send_the_queue_and_skips_only_the_current_song() {
        let start = Instant::now();
        let mut outbox = Outbox::default();
        let mut queue = queue_of(3);
        let mut state = HostState {
            queue: queue.view(),
            ..HostState::default()
        };
        outbox.updates(&state, start);
        queue.next(Advance::Manual);
        state.queue = queue.view();
        assert_eq!(kinds(&outbox.updates(&state, start)), ["queue_current"]);
        let more = queue_of(5);
        state.queue = more.view();
        assert_eq!(kinds(&outbox.updates(&state, start)), ["queue"]);
        state.volume = 40;
        state.is_shuffled = true;
        assert_eq!(kinds(&outbox.updates(&state, start)), ["volume", "modes"]);
    }

    #[test]
    fn remote_queues_follow_the_display_order() {
        let mut queue = queue_of(4);
        queue.play_uid(3);
        let remote = remote_queue(&queue.view());
        let uids: Vec<Uid> = remote.items.iter().map(|item| item.uid).collect();
        assert_eq!(uids, [1, 2, 3, 4]);
        assert_eq!(remote.current_uid, Some(3));
    }
}
