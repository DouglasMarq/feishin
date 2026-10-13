use std::collections::HashSet;

use super::support::{
    assert_invariants, current_id, item_ids, numbered_songs, play_ids, queue_playing, songs,
};
use crate::queue::{Advance, Queue, RepeatMode, Uid};

#[test]
fn add_next_without_shuffle_inserts_after_current() {
    let mut queue = queue_playing(&["a", "b", "c"], "b", RepeatMode::None);
    queue.add_next(songs(&["x", "y"]));
    assert_eq!(play_ids(&queue), ["a", "b", "x", "y", "c"]);
    assert_eq!(current_id(&queue).as_deref(), Some("b"));
    assert_eq!(
        queue
            .next(Advance::Manual)
            .map(|item| item.song.id.as_str()),
        Some("x")
    );
    assert_invariants(&queue);
}

#[test]
fn add_next_without_current_inserts_at_front() {
    let mut queue = Queue::with_seed(16);
    queue.add_last(songs(&["a", "b"]));
    queue.add_next(songs(&["x"]));
    assert_eq!(play_ids(&queue), ["x", "a", "b"]);
}

#[test]
fn add_next_with_shuffle_inserts_after_current_in_play_order() {
    let mut queue = Queue::with_seed(17);
    queue.set_shuffle(true);
    queue.play_now(numbered_songs(8), Some("s3"));
    queue.next(Advance::Manual);
    queue.next(Advance::Manual);
    let before = play_ids(&queue);
    let position = queue.current_position().unwrap_or_default();
    let current = current_id(&queue);

    queue.add_next(songs(&["x", "y", "z"]));
    let after = play_ids(&queue);
    assert_eq!(current_id(&queue), current);
    assert_eq!(queue.current_position(), Some(position));
    assert_eq!(after[..=position], before[..=position]);
    let inserted: HashSet<&str> = after[position + 1..position + 4]
        .iter()
        .map(String::as_str)
        .collect();
    assert_eq!(inserted, HashSet::from(["x", "y", "z"]));
    assert_eq!(after[position + 4..], before[position + 1..]);

    let current_index = item_ids(&queue)
        .iter()
        .position(|id| Some(id) == current.as_ref());
    let original = item_ids(&queue);
    let index = current_index.unwrap_or_default();
    assert_eq!(original[index + 1..index + 4], ["x", "y", "z"]);
    assert_invariants(&queue);

    queue.set_shuffle(false);
    assert_eq!(item_ids(&queue), original);
}

#[test]
fn add_last_without_shuffle_appends() {
    let mut queue = queue_playing(&["a", "b"], "a", RepeatMode::None);
    queue.add_last(songs(&["x", "y"]));
    assert_eq!(play_ids(&queue), ["a", "b", "x", "y"]);
    assert_eq!(current_id(&queue).as_deref(), Some("a"));
    assert_invariants(&queue);
}

#[test]
fn add_last_with_shuffle_appends_to_play_order() {
    let mut queue = Queue::with_seed(18);
    queue.set_shuffle(true);
    queue.play_now(numbered_songs(6), Some("s1"));
    let before = play_ids(&queue);
    queue.add_last(songs(&["x", "y", "z"]));
    let after = play_ids(&queue);
    assert_eq!(after[..6], before[..]);
    let tail: HashSet<&str> = after[6..].iter().map(String::as_str).collect();
    assert_eq!(tail, HashSet::from(["x", "y", "z"]));
    assert_eq!(item_ids(&queue)[6..], ["x", "y", "z"]);
    assert_eq!(current_id(&queue).as_deref(), Some("s1"));
    assert_invariants(&queue);
}

#[test]
fn duplicate_songs_get_distinct_increasing_uids() {
    let mut queue = Queue::with_seed(24);
    queue.play_now(songs(&["a", "a", "b"]), Some("a"));
    queue.add_last(songs(&["a"]));
    queue.add_next(songs(&["a"]));
    let uids: Vec<Uid> = queue.items().iter().map(|item| item.uid).collect();
    assert_eq!(uids.iter().collect::<HashSet<_>>().len(), 5);
    assert_eq!(current_id(&queue).as_deref(), Some("a"));
    assert_eq!(queue.current_position(), Some(0));

    let highest_uid = uids.iter().max().copied().unwrap_or_default();
    queue.play_now(songs(&["a"]), None);
    assert!(queue.items()[0].uid > highest_uid);
}
