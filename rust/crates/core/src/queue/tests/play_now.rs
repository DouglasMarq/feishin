use std::collections::HashSet;

use super::support::{
    assert_invariants, current_id, item_ids, numbered_ids, numbered_songs, play_ids, play_uids,
    songs,
};
use crate::queue::Queue;

#[test]
fn play_now_without_shuffle_keeps_order_and_starts_at_song() {
    let mut queue = Queue::with_seed(3);
    let current = queue
        .play_now(songs(&["a", "b", "c", "d"]), Some("c"))
        .map(|item| item.song.id.clone());
    assert_eq!(current.as_deref(), Some("c"));
    assert_eq!(play_ids(&queue), ["a", "b", "c", "d"]);
    assert_eq!(item_ids(&queue), ["a", "b", "c", "d"]);
    assert_eq!(queue.current_position(), Some(2));
    assert_invariants(&queue);
}

#[test]
fn play_now_falls_back_to_first_song() {
    let mut queue = Queue::with_seed(4);
    queue.play_now(songs(&["a", "b"]), Some("missing"));
    assert_eq!(current_id(&queue).as_deref(), Some("a"));
    queue.play_now(songs(&["x", "y"]), None);
    assert_eq!(current_id(&queue).as_deref(), Some("x"));
    assert_eq!(queue.current_position(), Some(0));
}

#[test]
fn play_now_replaces_queue() {
    let mut queue = Queue::with_seed(5);
    queue.play_now(songs(&["a", "b", "c"]), Some("b"));
    queue.play_now(songs(&["x", "y"]), Some("y"));
    assert_eq!(item_ids(&queue), ["x", "y"]);
    assert_eq!(current_id(&queue).as_deref(), Some("y"));
    assert_invariants(&queue);
}

#[test]
fn play_now_with_shuffle_reshuffles_every_call_with_start_song_first() {
    let mut queue = Queue::with_seed(6);
    queue.set_shuffle(true);
    let mut orders = HashSet::new();
    for _ in 0..10 {
        let current = queue
            .play_now(numbered_songs(20), Some("s7"))
            .map(|item| item.song.id.clone());
        assert_eq!(current.as_deref(), Some("s7"));
        assert_eq!(play_ids(&queue)[0], "s7");
        assert_eq!(queue.current_position(), Some(0));
        assert_eq!(item_ids(&queue), numbered_ids(20));
        assert_invariants(&queue);
        orders.insert(play_ids(&queue));
    }
    assert!(orders.len() > 1, "play_now must reshuffle on every call");
}

#[test]
fn play_now_with_shuffle_and_no_start_uses_first_in_play_order() {
    let mut queue = Queue::with_seed(7);
    queue.set_shuffle(true);
    let current = queue
        .play_now(numbered_songs(10), None)
        .map(|item| item.uid);
    assert_eq!(current, Some(play_uids(&queue)[0]));
    assert_eq!(queue.current_position(), Some(0));
}
