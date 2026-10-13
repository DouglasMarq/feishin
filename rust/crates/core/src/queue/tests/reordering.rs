use super::support::{
    assert_invariants, current_id, current_uid, item_ids, numbered_ids, numbered_songs, play_ids,
    play_uids, queue_playing, uid_of,
};
use crate::queue::{Queue, RepeatMode};

#[test]
fn move_items_without_shuffle() {
    let mut queue = queue_playing(&["a", "b", "c", "d", "e"], "c", RepeatMode::None);
    let (uid_a, uid_c, uid_e) = (
        uid_of(&queue, "a"),
        uid_of(&queue, "c"),
        uid_of(&queue, "e"),
    );
    queue.move_items(&[uid_c, uid_a], Some(uid_e));
    assert_eq!(item_ids(&queue), ["b", "d", "a", "c", "e"]);
    assert_eq!(current_id(&queue).as_deref(), Some("c"));
    assert_eq!(queue.current_position(), Some(3));

    queue.move_items(&[uid_a], None);
    assert_eq!(item_ids(&queue), ["b", "d", "c", "e", "a"]);

    let uid_b = uid_of(&queue, "b");
    queue.move_items(&[uid_e, uid_a], Some(uid_b));
    assert_eq!(item_ids(&queue), ["e", "a", "b", "d", "c"]);
    assert_invariants(&queue);
}

#[test]
fn move_items_before_one_of_the_moved_items() {
    let mut queue = queue_playing(&["a", "b", "c", "d", "e"], "a", RepeatMode::None);
    let (uid_b, uid_c) = (uid_of(&queue, "b"), uid_of(&queue, "c"));
    let uid_e = uid_of(&queue, "e");
    queue.move_items(&[uid_e, uid_b], Some(uid_b));
    assert_eq!(item_ids(&queue), ["a", "b", "e", "c", "d"]);
    queue.move_items(&[uid_c], Some(uid_c));
    assert_eq!(item_ids(&queue), ["a", "b", "e", "c", "d"]);
}

#[test]
fn move_items_ignores_unknown_uids() {
    let mut queue = queue_playing(&["a", "b", "c"], "a", RepeatMode::None);
    let uid_a = uid_of(&queue, "a");
    queue.move_items(&[uid_a], Some(4242));
    assert_eq!(item_ids(&queue), ["a", "b", "c"]);
    queue.move_items(&[4242], None);
    assert_eq!(item_ids(&queue), ["a", "b", "c"]);
}

#[test]
fn move_items_with_shuffle_only_reorders_play_order() {
    let mut queue = Queue::with_seed(23);
    queue.set_shuffle(true);
    queue.play_now(numbered_songs(8), Some("s4"));
    let play_order = play_uids(&queue);
    let current = current_uid(&queue);

    queue.move_items(&[play_order[6], play_order[2]], Some(play_order[1]));
    let mut expected = vec![play_order[0], play_order[2], play_order[6], play_order[1]];
    expected.extend([play_order[3], play_order[4], play_order[5], play_order[7]]);
    assert_eq!(play_uids(&queue), expected);
    assert_eq!(item_ids(&queue), numbered_ids(8));
    assert_eq!(current_uid(&queue), current);
    assert_eq!(queue.current_position(), Some(0));

    queue.move_items(&[play_order[0]], None);
    assert_eq!(play_uids(&queue).last().copied(), Some(play_order[0]));
    assert_eq!(queue.current_position(), Some(7));
    assert_invariants(&queue);

    queue.set_shuffle(false);
    assert_eq!(play_ids(&queue), numbered_ids(8));
}
