use std::collections::HashSet;
use std::ops::Range;

use super::support::{
    assert_invariants, current_id, current_uid, item_ids, numbered_ids, numbered_songs, play_ids,
    play_uids, songs, uid_of,
};
use crate::model::Song;
use crate::queue::{Advance, InsertAnchor, Queue, Uid};

fn page(songs: &[Song], range: Range<usize>) -> &[Song] {
    &songs[range]
}

fn owns(ranges: &[Range<Uid>]) -> impl Fn(Uid) -> bool + '_ {
    move |uid| ranges.iter().any(|range| range.contains(&uid))
}

fn own_range(queue: &Queue, first: &str, last: &str) -> Range<Uid> {
    uid_of(queue, first)..uid_of(queue, last) + 1
}

fn uids_of(range: Range<Uid>) -> Vec<Uid> {
    range.collect()
}

fn sorted(mut ids: Vec<String>) -> Vec<String> {
    ids.sort();
    ids
}

#[test]
fn play_now_at_starts_at_the_given_index() {
    let mut queue = Queue::with_seed(1);
    queue.play_now_at(numbered_songs(5), Some(3));
    assert_eq!(current_id(&queue).as_deref(), Some("s3"));
    queue.play_now_at(numbered_songs(5), Some(9));
    assert_eq!(current_id(&queue).as_deref(), Some("s0"));
    assert_invariants(&queue);
}

#[test]
fn anchored_inserts_land_next_to_their_anchor() {
    let mut queue = Queue::with_seed(2);
    queue.play_now(songs(&["a", "b", "c"]), Some("a"));
    let after_b = queue.insert_at_anchor(
        &songs(&["x", "y"]),
        InsertAnchor::After(uid_of(&queue, "b")),
    );
    assert_eq!(item_ids(&queue), ["a", "b", "x", "y", "c"]);
    assert_eq!(uids_of(after_b), [uid_of(&queue, "x"), uid_of(&queue, "y")]);
    queue.insert_at_anchor(&songs(&["p"]), InsertAnchor::Before(uid_of(&queue, "a")));
    queue.insert_at_anchor(&songs(&["start"]), InsertAnchor::Start);
    queue.insert_at_anchor(&songs(&["end"]), InsertAnchor::End);
    assert_eq!(
        item_ids(&queue),
        ["start", "p", "a", "b", "x", "y", "c", "end"]
    );
    assert_eq!(current_id(&queue).as_deref(), Some("a"));
    assert_invariants(&queue);
}

#[test]
fn missing_anchors_fall_back_to_the_queue_ends() {
    let mut queue = Queue::with_seed(3);
    queue.play_now(songs(&["a", "b"]), Some("a"));
    let missing = Uid::MAX;
    assert!(!queue.contains(missing));
    assert!(queue.contains(uid_of(&queue, "a")));
    queue.insert_at_anchor(&songs(&["after"]), InsertAnchor::After(missing));
    queue.insert_interleaved(&songs(&["before"]), InsertAnchor::Before(missing), |_| true);
    assert_eq!(item_ids(&queue), ["before", "a", "b", "after"]);
    let empty = queue.insert_at_anchor(&[], InsertAnchor::End);
    assert!(empty.is_empty());
    assert_invariants(&queue);
}

#[test]
fn anchored_inserts_follow_the_anchor_in_shuffled_play_order() {
    let mut queue = Queue::with_seed(4);
    queue.set_shuffle(true);
    queue.play_now(numbered_songs(10), Some("s4"));
    let current = current_uid(&queue).unwrap_or(Uid::MAX);
    let added = queue.insert_at_anchor(&songs(&["x", "y"]), InsertAnchor::After(current));
    let order = play_uids(&queue);
    assert_eq!(order[0], current);
    let following: HashSet<Uid> = order[1..3].iter().copied().collect();
    assert_eq!(following, uids_of(added).into_iter().collect());
    assert_eq!(&item_ids(&queue)[5..7], ["x", "y"]);
    assert_invariants(&queue);
}

#[test]
fn streamed_pages_rebuild_the_list_order_around_the_start_page() {
    let list = numbered_songs(9);
    let mut queue = Queue::with_seed(5);
    queue.play_now_at(page(&list, 3..6).to_vec(), Some(1));
    let start: Vec<Uid> = play_uids(&queue);
    let tail = start.last().copied().unwrap_or(Uid::MAX);
    let head = start.first().copied().unwrap_or(Uid::MAX);
    queue.insert_interleaved(page(&list, 6..9), InsertAnchor::After(tail), |_| true);
    queue.insert_interleaved(page(&list, 0..3), InsertAnchor::Before(head), |_| true);
    assert_eq!(play_ids(&queue), numbered_ids(9));
    assert_eq!(current_id(&queue).as_deref(), Some("s4"));
    assert_invariants(&queue);
}

#[test]
fn shuffled_streams_shuffle_the_whole_list_with_the_clicked_song_first() {
    let list = numbered_songs(40);
    let mut queue = Queue::with_seed(6);
    queue.set_shuffle(true);
    queue.play_now_at(page(&list, 10..20).to_vec(), Some(5));
    let mut own = vec![own_range(&queue, "s10", "s19")];
    let tail = uid_of(&queue, "s19");
    let after =
        queue.insert_interleaved(page(&list, 20..40), InsertAnchor::After(tail), owns(&own));
    own.push(after);
    let head = uid_of(&queue, "s10");
    queue.insert_interleaved(page(&list, 0..10), InsertAnchor::Before(head), owns(&own));

    let order = play_ids(&queue);
    assert_eq!(order[0], "s15");
    assert_eq!(sorted(order.clone()), sorted(numbered_ids(40)));
    assert_ne!(order, numbered_ids(40));
    assert_eq!(item_ids(&queue), numbered_ids(40));
    queue.set_shuffle(false);
    assert_eq!(play_ids(&queue), numbered_ids(40));
    assert_invariants(&queue);
}

#[test]
fn shuffled_streams_leave_foreign_items_where_the_user_put_them() {
    let list = numbered_songs(30);
    let mut queue = Queue::with_seed(7);
    queue.set_shuffle(true);
    queue.play_now_at(page(&list, 0..10).to_vec(), Some(0));
    let own = vec![own_range(&queue, "s0", "s9")];
    queue.add_next(songs(&["next"]));
    queue.add_last(songs(&["last"]));
    queue.insert_interleaved(
        page(&list, 10..30),
        InsertAnchor::After(uid_of(&queue, "s9")),
        owns(&own),
    );

    let order = play_ids(&queue);
    assert_eq!(order[0], "s0");
    assert_eq!(order[1], "next");
    assert_eq!(order.last().map(String::as_str), Some("last"));
    assert_eq!(order.len(), 32);
    assert_invariants(&queue);
}

#[test]
fn shuffled_streams_only_fill_positions_that_are_still_to_come() {
    let list = numbered_songs(20);
    let mut queue = Queue::with_seed(8);
    queue.set_shuffle(true);
    queue.play_now_at(page(&list, 0..10).to_vec(), Some(0));
    queue.next(Advance::Manual);
    queue.next(Advance::Manual);
    let played: Vec<String> = play_ids(&queue)[..3].to_vec();
    let own = vec![own_range(&queue, "s0", "s9")];
    queue.insert_interleaved(page(&list, 10..20), InsertAnchor::End, owns(&own));
    assert_eq!(play_ids(&queue)[..3], played);
    assert_eq!(queue.current_position(), Some(2));
    assert_invariants(&queue);
}

#[test]
fn shuffled_streams_place_every_song_uniformly_after_the_clicked_one() {
    let list = numbered_songs(6);
    let rounds = 6000;
    let mut first_after_clicked = std::collections::HashMap::<String, usize>::new();
    for seed in 0..rounds {
        let mut queue = Queue::with_seed(seed);
        queue.set_shuffle(true);
        queue.play_now_at(page(&list, 0..2).to_vec(), Some(0));
        let mut own = vec![own_range(&queue, "s0", "s1")];
        let tail = uid_of(&queue, "s1");
        own.push(queue.insert_interleaved(
            page(&list, 2..4),
            InsertAnchor::After(tail),
            owns(&own),
        ));
        let tail = uid_of(&queue, "s3");
        queue.insert_interleaved(page(&list, 4..6), InsertAnchor::After(tail), owns(&own));
        *first_after_clicked
            .entry(play_ids(&queue)[1].clone())
            .or_default() += 1;
    }
    let expected = rounds as usize / 5;
    assert_eq!(first_after_clicked.len(), 5);
    for count in first_after_clicked.values() {
        assert!(
            count.abs_diff(expected) < expected / 5,
            "{first_after_clicked:?}"
        );
    }
}
