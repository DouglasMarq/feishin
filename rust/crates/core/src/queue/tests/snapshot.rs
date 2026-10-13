use super::support::{
    assert_invariants, current_id, item_ids, numbered_songs, play_uids, song, songs,
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
                .all(|item| item.uid < snapshot.next_uid || item.song.id == "y")
        );
    }
}

fn items_with_duplicate_uids() -> Vec<QueueItem> {
    vec![
        QueueItem {
            uid: 5,
            song: song("a"),
        },
        QueueItem {
            uid: 5,
            song: song("b"),
        },
        QueueItem {
            uid: 9,
            song: song("c"),
        },
    ]
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
