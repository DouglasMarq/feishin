use super::support::{
    assert_invariants, current_id, item_ids, numbered_songs, play_uids, songs, track,
};
use crate::queue::{Advance, Queue, QueueItem, QueueSnapshot, RepeatMode};

#[test]
fn snapshot_round_trip() {
    for shuffle in [false, true] {
        let mut queue = Queue::with_seed(26);
        queue.set_shuffle(shuffle);
        queue.set_repeat(RepeatMode::One);
        queue.play_now(numbered_songs(7), Some("s5"));
        queue.next(Advance::Manual);
        queue.add_last(songs(&["x"]));

        let snapshot = queue.snapshot();
        assert_eq!(snapshot.shuffled.is_some(), shuffle);
        let json = serde_json::to_string(&snapshot).unwrap_or_default();
        let decoded: QueueSnapshot =
            serde_json::from_str(&json).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(decoded, snapshot);

        let mut restored = Queue::from_snapshot(decoded);
        assert_eq!(restored.snapshot(), snapshot);
        assert_eq!(play_uids(&restored), play_uids(&queue));
        assert_eq!(restored.current(), queue.current());
        assert_eq!(restored.repeat(), RepeatMode::One);
        assert_eq!(restored.shuffle(), shuffle);

        restored.add_last(songs(&["y"]));
        assert_invariants(&restored);
        assert!(
            restored
                .items()
                .iter()
                .all(|item| item.uid < snapshot.next_uid || item.track.song_id == "y")
        );
    }
}

fn items_with_duplicate_uids() -> Vec<QueueItem> {
    vec![
        QueueItem {
            uid: 5,
            track: track("a").into(),
        },
        QueueItem {
            uid: 5,
            track: track("b").into(),
        },
        QueueItem {
            uid: 9,
            track: track("c").into(),
        },
    ]
}

#[test]
fn view_borrows_the_same_state_as_the_snapshot() {
    let mut queue = Queue::with_seed(3);
    queue.set_shuffle(true);
    queue.play_now(numbered_songs(5), Some("s2"));
    let snapshot = queue.snapshot();
    let view = queue.snapshot_ref();
    assert_eq!(view.items, snapshot.items.as_slice());
    assert_eq!(view.shuffled, snapshot.shuffled.as_deref());
    assert_eq!(view.current_uid, snapshot.current_uid);
    assert_eq!(view.repeat, snapshot.repeat);
    assert_eq!(view.next_uid, snapshot.next_uid);
    assert_eq!(snapshot.snapshot_ref(), view);
}

#[test]
fn from_snapshot_repairs_inconsistent_state() {
    let queue = Queue::from_snapshot(QueueSnapshot {
        items: items_with_duplicate_uids(),
        shuffled: Some(vec![0, 0, 7]),
        current_uid: Some(9),
        repeat: RepeatMode::All,
        next_uid: 2,
    });
    assert_invariants(&queue);
    assert!(queue.shuffle());
    assert_eq!(current_id(&queue).as_deref(), Some("c"));
    assert_eq!(queue.current_position(), Some(0));
    assert!(queue.snapshot().next_uid > 9);

    let queue = Queue::from_snapshot(QueueSnapshot {
        items: items_with_duplicate_uids(),
        shuffled: None,
        current_uid: Some(1234),
        repeat: RepeatMode::None,
        next_uid: 0,
    });
    assert_invariants(&queue);
    assert!(queue.current().is_none());
    assert_eq!(item_ids(&queue), ["a", "b", "c"]);
}
