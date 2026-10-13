use super::*;
use crate::demo::test_song;

fn state(ids: &[&str], position_ms: u64) -> PlayQueueState {
    PlayQueueState {
        song_ids: ids.iter().map(|id| (*id).to_owned()).collect(),
        current: ids.first().map(|id| (*id).to_owned()),
        position_ms,
    }
}

fn queue_of(count: usize, current: &str) -> Queue {
    let mut queue = Queue::with_seed(1);
    let songs = (0..count)
        .map(|index| test_song(&format!("s{index}"), 1000))
        .collect();
    queue.play_now(songs, Some(current));
    queue
}

#[test]
fn only_the_latest_changed_state_is_saved() {
    let mut shared = Shared {
        generation: 2,
        saved: None,
    };
    assert_eq!(shared.take_if_current(1, state(&["a"], 0)), None);
    assert_eq!(
        shared.take_if_current(2, state(&["a"], 0)),
        Some(state(&["a"], 0))
    );
    assert_eq!(shared.take_if_current(2, state(&["a"], 0)), None);
    assert!(shared.take_if_current(2, state(&["a"], 5)).is_some());
}

#[test]
fn state_follows_play_order_and_rejects_foreign_songs() {
    let queue = queue_of(3, "s1");
    let mut sync = PlayQueueSync::default();
    assert_eq!(
        sync.state_of(&queue, 700, "demo"),
        Some(PlayQueueState {
            current: Some("s1".into()),
            ..state(&["s0", "s1", "s2"], 700)
        })
    );
    assert_eq!(sync.state_of(&queue, 0, "other"), None);
    assert_eq!(
        sync.state_of(&Queue::with_seed(1), 900, "demo"),
        Some(state(&[], 0))
    );
}

#[test]
fn position_only_states_reuse_the_song_id_list() {
    let mut queue = queue_of(4, "s0");
    let mut sync = PlayQueueSync::default();
    let first = sync.state_of(&queue, 0, "demo").unwrap();
    queue.next(feishin_core::queue::Advance::Manual);
    let moved = sync.state_of(&queue, 5_000, "demo").unwrap();
    assert!(Arc::ptr_eq(&first.song_ids, &moved.song_ids));
    assert_eq!(moved.current.as_deref(), Some("s1"));

    queue.set_shuffle(true);
    let shuffled = sync.state_of(&queue, 5_000, "demo").unwrap();
    assert!(!Arc::ptr_eq(&first.song_ids, &shuffled.song_ids));
    assert_eq!(shuffled.song_ids.first().map(String::as_str), Some("s1"));
}

#[test]
fn a_replaced_queue_never_reuses_stale_song_ids() {
    let mut sync = PlayQueueSync::default();
    let empty = Queue::with_seed(1);
    assert_eq!(sync.state_of(&empty, 0, "demo"), Some(state(&[], 0)));
    let restored = Queue::from_snapshot(queue_of(2, "s0").snapshot());
    assert_eq!(restored.revision(), empty.revision());
    let state = sync.state_of(&restored, 0, "demo").unwrap();
    assert_eq!(state.song_ids.len(), 2);
}

#[test]
fn generations_track_the_latest_schedule() {
    let sync = PlayQueueSync::default();
    let first = sync.next_generation();
    assert!(sync.is_current(first));
    let second = sync.next_generation();
    assert!(!sync.is_current(first));
    assert!(sync.is_current(second));
}
