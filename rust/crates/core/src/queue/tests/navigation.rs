use super::support::{current_id, current_uid, numbered_songs, play_uids, queue_playing, uid_of};
use crate::queue::{Advance, Queue, RepeatMode};

#[test]
fn next_with_repeat_none() {
    for advance in [Advance::Auto, Advance::Manual] {
        let mut queue = queue_playing(&["a", "b", "c"], "a", RepeatMode::None);
        assert_eq!(
            queue.next(advance).map(|item| item.track.song_id.as_str()),
            Some("b")
        );
        assert_eq!(
            queue.next(advance).map(|item| item.track.song_id.as_str()),
            Some("c")
        );
        assert!(queue.next(advance).is_none());
        assert_eq!(current_id(&queue).as_deref(), Some("a"));
    }
}

#[test]
fn next_with_repeat_all_wraps() {
    for advance in [Advance::Auto, Advance::Manual] {
        let mut queue = queue_playing(&["a", "b", "c"], "b", RepeatMode::All);
        assert_eq!(
            queue.next(advance).map(|item| item.track.song_id.as_str()),
            Some("c")
        );
        assert_eq!(
            queue.next(advance).map(|item| item.track.song_id.as_str()),
            Some("a")
        );
        assert_eq!(current_id(&queue).as_deref(), Some("a"));
    }
}

#[test]
fn next_with_repeat_one_repeats_only_on_auto() {
    let mut queue = queue_playing(&["a", "b", "c"], "b", RepeatMode::One);
    let uid = current_uid(&queue);
    assert_eq!(queue.next(Advance::Auto).map(|item| item.uid), uid);
    assert_eq!(queue.next(Advance::Auto).map(|item| item.uid), uid);
    assert_eq!(
        queue
            .next(Advance::Manual)
            .map(|item| item.track.song_id.as_str()),
        Some("c")
    );
    assert_eq!(
        queue
            .next(Advance::Manual)
            .map(|item| item.track.song_id.as_str()),
        Some("c")
    );
    assert_eq!(
        queue
            .next(Advance::Auto)
            .map(|item| item.track.song_id.as_str()),
        Some("c")
    );
}

#[test]
fn previous_with_every_repeat_mode() {
    for repeat in [RepeatMode::None, RepeatMode::All, RepeatMode::One] {
        let mut queue = queue_playing(&["a", "b", "c"], "c", repeat);
        assert_eq!(
            queue.previous().map(|item| item.track.song_id.as_str()),
            Some("b")
        );
        assert_eq!(
            queue.previous().map(|item| item.track.song_id.as_str()),
            Some("a")
        );
        let expected = match repeat {
            RepeatMode::All => "c",
            RepeatMode::None | RepeatMode::One => "a",
        };
        assert_eq!(
            queue.previous().map(|item| item.track.song_id.as_str()),
            Some(expected)
        );
        assert_eq!(current_id(&queue).as_deref(), Some(expected));
    }
}

#[test]
fn single_item_queue_navigation() {
    let mut queue = queue_playing(&["a"], "a", RepeatMode::None);
    assert!(queue.peek_next().is_none());
    assert!(queue.next(Advance::Manual).is_none());
    assert_eq!(current_id(&queue).as_deref(), Some("a"));
    queue.set_repeat(RepeatMode::All);
    assert_eq!(
        queue
            .next(Advance::Auto)
            .map(|item| item.track.song_id.as_str()),
        Some("a")
    );
    assert_eq!(
        queue.previous().map(|item| item.track.song_id.as_str()),
        Some("a")
    );
    queue.set_repeat(RepeatMode::One);
    assert_eq!(
        queue
            .next(Advance::Manual)
            .map(|item| item.track.song_id.as_str()),
        Some("a")
    );
}

#[test]
fn next_and_previous_follow_shuffled_order() {
    let mut queue = Queue::with_seed(14);
    queue.set_shuffle(true);
    queue.play_now(numbered_songs(10), Some("s0"));
    let play_order = play_uids(&queue);
    for expected in &play_order[1..] {
        assert_eq!(
            queue.next(Advance::Manual).map(|item| item.uid),
            Some(*expected)
        );
    }
    assert!(queue.next(Advance::Auto).is_none());
    assert_eq!(current_uid(&queue), Some(play_order[0]));

    queue.set_repeat(RepeatMode::All);
    assert_eq!(
        queue.previous().map(|item| item.uid),
        play_order.last().copied()
    );
    assert_eq!(queue.previous().map(|item| item.uid), Some(play_order[8]));
    assert_eq!(
        queue.next(Advance::Auto).map(|item| item.uid),
        Some(play_order[9])
    );
    assert_eq!(
        queue.next(Advance::Auto).map(|item| item.uid),
        Some(play_order[0])
    );
}

#[test]
fn peek_next_always_matches_next_auto() {
    for is_shuffled in [false, true] {
        for repeat in [RepeatMode::None, RepeatMode::All, RepeatMode::One] {
            let mut queue = Queue::with_seed(15);
            queue.set_shuffle(is_shuffled);
            queue.play_now(numbered_songs(6), Some("s2"));
            queue.set_repeat(repeat);
            for uid in play_uids(&queue) {
                queue.play_uid(uid);
                let peeked = queue.peek_next().cloned();
                let mut advanced = queue.clone();
                assert_eq!(advanced.next(Advance::Auto).cloned(), peeked);
            }
        }
    }
}

#[test]
fn play_uid_selects_known_items_only() {
    let mut queue = queue_playing(&["a", "b", "c"], "a", RepeatMode::None);
    let uid_c = uid_of(&queue, "c");
    assert_eq!(queue.play_uid(uid_c).map(|item| item.uid), Some(uid_c));
    assert!(queue.play_uid(9999).is_none());
    assert_eq!(current_id(&queue).as_deref(), Some("c"));
}
