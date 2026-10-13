use super::support::{assert_invariants, play_uids, songs};
use crate::queue::{Advance, Queue};

#[test]
fn empty_queue_edge_cases() {
    let mut queue = Queue::with_seed(1);
    assert!(queue.is_empty());
    assert_eq!(queue.len(), 0);
    assert!(queue.current().is_none());
    assert_eq!(queue.current_position(), None);
    assert!(queue.display_order().is_empty());
    assert!(queue.peek_next().is_none());
    assert!(queue.next(Advance::Auto).is_none());
    assert!(queue.next(Advance::Manual).is_none());
    assert!(queue.previous().is_none());
    assert!(queue.play_uid(1).is_none());
    queue.remove(&[1, 2]);
    queue.move_items(&[1], None);
    queue.move_items(&[1], Some(2));
    queue.clear();
    assert!(queue.play_now(Vec::new(), Some("x")).is_none());
    queue.set_shuffle(true);
    assert!(queue.shuffle());
    assert!(queue.play_now(Vec::new(), None).is_none());
    assert!(queue.next(Advance::Manual).is_none());
    assert!(queue.previous().is_none());
    assert_invariants(&queue);
}

#[test]
fn adding_to_empty_queue_leaves_nothing_current_until_next() {
    for shuffle in [false, true] {
        let mut queue = Queue::with_seed(2);
        queue.set_shuffle(shuffle);
        queue.add_last(songs(&["a", "b", "c"]));
        assert!(queue.current().is_none());
        assert_eq!(
            queue.peek_next().map(|item| item.uid),
            Some(play_uids(&queue)[0])
        );
        let first = play_uids(&queue)[0];
        assert_eq!(queue.next(Advance::Auto).map(|item| item.uid), Some(first));
        assert_invariants(&queue);

        let mut queue = Queue::with_seed(2);
        queue.set_shuffle(shuffle);
        queue.add_next(songs(&["a", "b"]));
        assert!(queue.current().is_none());
        let first = play_uids(&queue)[0];
        assert_eq!(queue.previous().map(|item| item.uid), Some(first));
        assert_invariants(&queue);
    }
}
