use std::sync::Arc;

use super::support::{numbered_songs, play_uids, uid_of};
use crate::queue::{Advance, Queue};

fn playing_queue(count: usize) -> Queue {
    let mut queue = Queue::with_seed(5);
    queue.play_now(numbered_songs(count), Some("s0"));
    queue
}

#[test]
fn track_changes_reuse_the_published_layout() {
    let mut queue = playing_queue(4);
    let before = queue.view();
    queue.next(Advance::Manual);
    let after = queue.view();
    assert!(after.shares_layout_with(&before));
    assert_eq!(before.current_position(), Some(0));
    assert_eq!(after.current_position(), Some(1));
    assert_eq!(
        after.current().map(|item| item.uid),
        Some(uid_of(&queue, "s1"))
    );
}

#[test]
fn structural_edits_publish_a_new_layout_and_keep_old_views_intact() {
    let mut queue = playing_queue(4);
    let before = queue.view();
    queue.remove(&[uid_of(&queue, "s2")]);
    let after = queue.view();
    assert!(!after.shares_layout_with(&before));
    assert_eq!(before.len(), 4);
    assert_eq!(after.len(), 3);
    assert_ne!(after.revision(), before.revision());
    assert_eq!(after.uids().collect::<Vec<_>>(), play_uids(&queue));
}

#[test]
fn shuffled_view_follows_the_play_order() {
    let mut queue = playing_queue(50);
    queue.set_shuffle(true);
    queue.next(Advance::Manual);
    queue.next(Advance::Manual);
    let view = queue.view();
    assert_eq!(view.uids().collect::<Vec<_>>(), play_uids(&queue));
    assert_eq!(view.current_position(), Some(2));
    assert_eq!(
        view.item_at(2).map(|item| item.uid),
        queue.current().map(|item| item.uid)
    );
    assert!(view.item_at(50).is_none());
}

#[test]
fn view_shares_tracks_with_the_queue() {
    let queue = playing_queue(3);
    let view = queue.view();
    let shared = view
        .item_at(1)
        .zip(queue.items().get(1))
        .is_some_and(|(viewed, queued)| Arc::ptr_eq(&viewed.track, &queued.track));
    assert!(shared);
}

#[test]
fn item_missing_from_the_play_order_has_no_position() {
    let mut queue = playing_queue(3);
    queue.shuffled_play_order = Some(vec![1, 2]);
    queue.bump_revision();
    assert_eq!(queue.current_position(), None);
    assert_eq!(queue.view().current_position(), None);
    assert!(queue.view().current().is_none());
}

#[test]
fn empty_queue_has_an_empty_view() {
    let view = Queue::with_seed(1).view();
    assert!(view.is_empty());
    assert_eq!(view.current_position(), None);
    assert!(view.current().is_none());
}
