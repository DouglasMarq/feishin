use std::collections::HashSet;

use super::support::{
    assert_invariants, current_id, current_uid, item_ids, numbered_ids, numbered_songs, play_ids,
    play_uids,
};
use crate::queue::{Advance, Queue};

#[test]
fn shuffle_on_puts_current_first() {
    let mut queue = Queue::with_seed(8);
    queue.play_now(numbered_songs(15), Some("s9"));
    let uid = current_uid(&queue);
    queue.set_shuffle(true);
    assert!(queue.shuffle());
    assert_eq!(current_uid(&queue), uid);
    assert_eq!(queue.current_position(), Some(0));
    assert_eq!(item_ids(&queue), numbered_ids(15));
    assert_invariants(&queue);
}

#[test]
fn shuffle_on_without_current_shuffles_everything() {
    let mut queue = Queue::with_seed(9);
    queue.add_last(numbered_songs(30));
    queue.set_shuffle(true);
    assert!(queue.current().is_none());
    assert_ne!(play_ids(&queue), numbered_ids(30));
    assert_invariants(&queue);
}

#[test]
fn shuffle_off_restores_original_order_and_keeps_current() {
    let mut queue = Queue::with_seed(10);
    queue.set_shuffle(true);
    queue.play_now(numbered_songs(12), Some("s3"));
    queue.next(Advance::Manual);
    queue.next(Advance::Manual);
    let uid_before = current_uid(&queue);
    let id_before = current_id(&queue);
    assert_ne!(play_ids(&queue), numbered_ids(12));

    queue.set_shuffle(false);
    assert!(!queue.shuffle());
    assert_eq!(play_ids(&queue), numbered_ids(12));
    assert_eq!(current_uid(&queue), uid_before);
    let expected_position = numbered_ids(12)
        .iter()
        .position(|id| Some(id) == id_before.as_ref());
    assert_eq!(queue.current_position(), expected_position);
    assert_invariants(&queue);
}

#[test]
fn set_shuffle_is_idempotent() {
    let mut queue = Queue::with_seed(11);
    queue.play_now(numbered_songs(10), Some("s4"));
    queue.set_shuffle(true);
    let play_order = play_uids(&queue);
    queue.set_shuffle(true);
    assert_eq!(play_uids(&queue), play_order);
    queue.set_shuffle(false);
    queue.set_shuffle(false);
    assert_eq!(play_ids(&queue), numbered_ids(10));
}

#[test]
fn toggling_shuffle_on_again_reshuffles_with_current_first() {
    let mut queue = Queue::with_seed(12);
    queue.play_now(numbered_songs(20), Some("s5"));
    let mut orders = HashSet::new();
    for _ in 0..5 {
        queue.set_shuffle(true);
        assert_eq!(current_id(&queue).as_deref(), Some("s5"));
        assert_eq!(queue.current_position(), Some(0));
        orders.insert(play_ids(&queue));
        queue.set_shuffle(false);
    }
    assert!(orders.len() > 1);
}

#[test]
fn with_seed_is_reproducible() {
    let shuffled_ids_for_seed = |seed| {
        let mut queue = Queue::with_seed(seed);
        queue.set_shuffle(true);
        queue.play_now(numbered_songs(30), Some("s3"));
        queue.add_next(numbered_songs(5));
        queue.add_last(numbered_songs(5));
        play_ids(&queue)
    };
    assert_eq!(shuffled_ids_for_seed(42), shuffled_ids_for_seed(42));
    assert_ne!(shuffled_ids_for_seed(42), shuffled_ids_for_seed(43));
}
