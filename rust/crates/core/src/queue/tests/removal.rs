use super::support::{
    assert_invariants, current_id, current_uid, item_ids, numbered_songs, play_ids, play_uids,
    queue_playing, songs, uid_of,
};
use crate::queue::{Advance, Queue, RepeatMode};

#[test]
fn remove_non_current_items() {
    for is_shuffled in [false, true] {
        let mut queue = Queue::with_seed(19);
        queue.set_shuffle(is_shuffled);
        queue.play_now(songs(&["a", "b", "c", "d"]), Some("b"));
        let before: Vec<String> = play_ids(&queue)
            .into_iter()
            .filter(|id| id != "a" && id != "d")
            .collect();
        let (uid_a, uid_d) = (uid_of(&queue, "a"), uid_of(&queue, "d"));
        queue.remove(&[uid_a, uid_d, 4242]);
        assert_eq!(item_ids(&queue), ["b", "c"]);
        assert_eq!(play_ids(&queue), before);
        assert_eq!(current_id(&queue).as_deref(), Some("b"));
        assert_invariants(&queue);
    }
}

#[test]
fn remove_current_moves_to_following_item_in_play_order() {
    for is_shuffled in [false, true] {
        let mut queue = Queue::with_seed(20);
        queue.set_shuffle(is_shuffled);
        queue.play_now(numbered_songs(8), Some("s0"));
        queue.next(Advance::Manual);
        queue.next(Advance::Manual);
        let play_order = play_uids(&queue);
        let position = queue.current_position().unwrap_or_default();
        queue.remove(&[play_order[position], play_order[position + 1]]);
        assert_eq!(current_uid(&queue), Some(play_order[position + 2]));
        assert_eq!(queue.current_position(), Some(position));
        assert_invariants(&queue);
    }
}

#[test]
fn remove_current_at_end_moves_to_first_item() {
    for is_shuffled in [false, true] {
        let mut queue = Queue::with_seed(21);
        queue.set_shuffle(is_shuffled);
        queue.play_now(numbered_songs(4), None);
        let play_order = play_uids(&queue);
        queue.play_uid(play_order[3]);
        queue.remove(&[play_order[3], play_order[0]]);
        assert_eq!(current_uid(&queue), Some(play_order[1]));
        assert_invariants(&queue);
    }
}

#[test]
fn remove_everything_leaves_no_current() {
    for is_shuffled in [false, true] {
        let mut queue = Queue::with_seed(22);
        queue.set_shuffle(is_shuffled);
        queue.play_now(numbered_songs(3), None);
        let all_uids = play_uids(&queue);
        queue.remove(&all_uids);
        assert!(queue.is_empty());
        assert!(queue.current().is_none());
        assert_eq!(queue.is_shuffled(), is_shuffled);
        assert_invariants(&queue);
    }
}

#[test]
fn removing_one_duplicate_keeps_the_other() {
    let mut queue = queue_playing(&["a", "b", "a"], "a", RepeatMode::None);
    let second = queue.items()[2].uid;
    queue.remove(&[second]);
    assert_eq!(item_ids(&queue), ["a", "b"]);
    assert_eq!(current_uid(&queue), Some(queue.items()[0].uid));
}

#[test]
fn clear_keeps_modes() {
    let mut queue = Queue::with_seed(25);
    queue.set_shuffle(true);
    queue.set_repeat(RepeatMode::All);
    queue.play_now(numbered_songs(5), None);
    queue.clear();
    assert!(queue.is_empty());
    assert!(queue.current().is_none());
    assert!(queue.is_shuffled());
    assert_eq!(queue.repeat(), RepeatMode::All);
    let current = queue
        .play_now(numbered_songs(3), Some("s1"))
        .map(|item| item.track.song_id.to_string());
    assert_eq!(current.as_deref(), Some("s1"));
    assert_eq!(queue.current_position(), Some(0));
}
