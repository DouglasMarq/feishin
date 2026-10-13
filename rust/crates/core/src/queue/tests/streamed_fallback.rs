use std::ops::Range;

use super::support::{assert_invariants, current_id, numbered_songs, play_ids, songs, uid_of};
use crate::queue::{Advance, InsertAnchor, Queue, Uid};

fn owns(ranges: &[Range<Uid>]) -> impl Fn(Uid) -> bool + '_ {
    move |uid| ranges.iter().any(|range| range.contains(&uid))
}

fn streamed_ids(from: usize, to: usize) -> Vec<String> {
    (from..to).map(|index| format!("s{index}")).collect()
}

fn sorted(mut ids: Vec<String>) -> Vec<String> {
    ids.sort();
    ids
}

fn queue_that_played_its_whole_first_page() -> (Queue, Range<Uid>) {
    let list = numbered_songs(2);
    let mut queue = Queue::with_seed(11);
    queue.set_shuffle(true);
    queue.play_now_at(list, Some(0));
    let own = uid_of(&queue, "s0")..uid_of(&queue, "s1") + 1;
    queue.add_last(songs(&["last"]));
    queue.next(Advance::Manual);
    (queue, own)
}

#[test]
fn shuffled_streams_that_ran_out_of_upcoming_songs_continue_after_the_current_one() {
    let (mut queue, own) = queue_that_played_its_whole_first_page();
    assert_eq!(current_id(&queue).as_deref(), Some("s1"));
    let next_page: Vec<_> = numbered_songs(10).into_iter().skip(2).collect();
    let tail = uid_of(&queue, "s1");
    queue.insert_interleaved(
        &next_page,
        InsertAnchor::After(tail),
        owns(std::slice::from_ref(&own)),
    );

    let order = play_ids(&queue);
    assert_eq!(order[..2], ["s0", "s1"]);
    assert_eq!(sorted(order[2..10].to_vec()), sorted(streamed_ids(2, 10)));
    assert_eq!(order.last().map(String::as_str), Some("last"));
    assert_invariants(&queue);
}

#[test]
fn shuffled_streams_whose_anchor_is_the_end_still_append() {
    let (mut queue, own) = queue_that_played_its_whole_first_page();
    let next_page: Vec<_> = numbered_songs(10).into_iter().skip(2).collect();
    queue.insert_interleaved(
        &next_page,
        InsertAnchor::End,
        owns(std::slice::from_ref(&own)),
    );

    let order = play_ids(&queue);
    assert_eq!(order[..3], ["s0", "s1", "last"]);
    assert_eq!(sorted(order[3..].to_vec()), sorted(streamed_ids(2, 10)));
    assert_invariants(&queue);
}

#[test]
fn shuffled_enqueued_selections_stay_together_and_mix_every_page() {
    let rounds = 3000;
    let mut later_page_first = 0;
    for seed in 0..rounds {
        let mut queue = Queue::with_seed(seed);
        queue.set_shuffle(true);
        queue.play_now(songs(&["current", "rest"]), Some("current"));
        let current = uid_of(&queue, "current");
        let selection = numbered_songs(4);
        let first_page = selection.get(..2).unwrap_or_default();
        let second_page = selection.get(2..).unwrap_or_default();
        let mut own = vec![queue.insert_at_anchor(first_page, InsertAnchor::After(current))];
        let tail = uid_of(&queue, "s1");
        own.push(queue.insert_interleaved(second_page, InsertAnchor::After(tail), owns(&own)));

        let order = play_ids(&queue);
        assert_eq!(order.first().map(String::as_str), Some("current"));
        assert_eq!(order.last().map(String::as_str), Some("rest"));
        later_page_first += usize::from(["s2", "s3"].contains(&order[1].as_str()));
    }
    let expected = rounds as usize / 2;
    assert!(
        later_page_first.abs_diff(expected) < expected / 10,
        "{later_page_first}"
    );
}
