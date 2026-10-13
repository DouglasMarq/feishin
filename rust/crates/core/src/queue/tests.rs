use std::collections::HashSet;

use proptest::prelude::*;

use super::*;

fn song(id: &str) -> Song {
    Song {
        id: id.to_owned(),
        server_id: "server".to_owned(),
        title: format!("Title {id}"),
        artists: Vec::new(),
        album_artists: Vec::new(),
        album: None,
        album_id: None,
        duration_ms: 180_000,
        track_number: None,
        disc_number: None,
        year: None,
        genres: Vec::new(),
        bitrate_kbps: None,
        play_count: 0,
        user_rating: None,
        favorite: false,
        cover_art_id: None,
        replay_gain: None,
        path: None,
        size_bytes: None,
        container: None,
        created_at: None,
        last_played_at: None,
    }
}

fn songs(ids: &[&str]) -> Vec<Song> {
    ids.iter().map(|id| song(id)).collect()
}

fn numbered(n: usize) -> Vec<Song> {
    (0..n).map(|i| song(&format!("s{i}"))).collect()
}

fn numbered_ids(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("s{i}")).collect()
}

fn play_ids(q: &Queue) -> Vec<String> {
    q.display_order()
        .iter()
        .map(|it| it.song.id.clone())
        .collect()
}

fn item_ids(q: &Queue) -> Vec<String> {
    q.items().iter().map(|it| it.song.id.clone()).collect()
}

fn play_uids(q: &Queue) -> Vec<Uid> {
    q.display_order().iter().map(|it| it.uid).collect()
}

fn current_id(q: &Queue) -> Option<String> {
    q.current().map(|it| it.song.id.clone())
}

fn uid_of(q: &Queue, id: &str) -> Uid {
    q.items()
        .iter()
        .find(|it| it.song.id == id)
        .map(|it| it.uid)
        .unwrap_or_else(|| panic!("no item with song id {id}"))
}

fn assert_invariants(q: &Queue) {
    let items: HashSet<Uid> = q.items().iter().map(|it| it.uid).collect();
    assert_eq!(items.len(), q.len(), "uids must be unique");
    let display = play_uids(q);
    assert_eq!(display.len(), q.len());
    assert_eq!(display.iter().copied().collect::<HashSet<_>>(), items);
    assert_eq!(q.is_empty(), display.is_empty());
    match q.current() {
        Some(cur) => {
            assert!(items.contains(&cur.uid));
            let pos = q.current_position().expect("current has a position");
            assert_eq!(display[pos], cur.uid);
        }
        None => assert_eq!(q.current_position(), None),
    }
    if let Some(order) = q.snapshot().shuffled {
        assert!(is_permutation(&order, q.len()));
    }
    assert!(q.snapshot().next_uid > q.items().iter().map(|it| it.uid).max().unwrap_or(0));
}

#[test]
fn empty_queue_edge_cases() {
    let mut q = Queue::with_seed(1);
    assert!(q.is_empty());
    assert_eq!(q.len(), 0);
    assert!(q.current().is_none());
    assert_eq!(q.current_position(), None);
    assert!(q.display_order().is_empty());
    assert!(q.peek_next().is_none());
    assert!(q.next(Advance::Auto).is_none());
    assert!(q.next(Advance::Manual).is_none());
    assert!(q.previous().is_none());
    assert!(q.play_uid(1).is_none());
    q.remove(&[1, 2]);
    q.move_items(&[1], None);
    q.move_items(&[1], Some(2));
    q.clear();
    assert!(q.play_now(Vec::new(), Some("x")).is_none());
    q.set_shuffle(true);
    assert!(q.shuffle());
    assert!(q.play_now(Vec::new(), None).is_none());
    assert!(q.next(Advance::Manual).is_none());
    assert!(q.previous().is_none());
    assert_invariants(&q);
}

#[test]
fn adding_to_empty_queue_leaves_nothing_current_until_next() {
    for shuffle in [false, true] {
        let mut q = Queue::with_seed(2);
        q.set_shuffle(shuffle);
        q.add_last(songs(&["a", "b", "c"]));
        assert!(q.current().is_none());
        assert_eq!(q.peek_next().map(|it| it.uid), Some(play_uids(&q)[0]));
        let first = play_uids(&q)[0];
        assert_eq!(q.next(Advance::Auto).map(|it| it.uid), Some(first));
        assert_invariants(&q);

        let mut q = Queue::with_seed(2);
        q.set_shuffle(shuffle);
        q.add_next(songs(&["a", "b"]));
        assert!(q.current().is_none());
        let first = play_uids(&q)[0];
        assert_eq!(q.previous().map(|it| it.uid), Some(first));
        assert_invariants(&q);
    }
}

#[test]
fn play_now_without_shuffle_keeps_order_and_starts_at_song() {
    let mut q = Queue::with_seed(3);
    let cur = q
        .play_now(songs(&["a", "b", "c", "d"]), Some("c"))
        .map(|it| it.song.id.clone());
    assert_eq!(cur.as_deref(), Some("c"));
    assert_eq!(play_ids(&q), ["a", "b", "c", "d"]);
    assert_eq!(item_ids(&q), ["a", "b", "c", "d"]);
    assert_eq!(q.current_position(), Some(2));
    assert_invariants(&q);
}

#[test]
fn play_now_falls_back_to_first_song() {
    let mut q = Queue::with_seed(4);
    q.play_now(songs(&["a", "b"]), Some("missing"));
    assert_eq!(current_id(&q).as_deref(), Some("a"));
    q.play_now(songs(&["x", "y"]), None);
    assert_eq!(current_id(&q).as_deref(), Some("x"));
    assert_eq!(q.current_position(), Some(0));
}

#[test]
fn play_now_replaces_queue() {
    let mut q = Queue::with_seed(5);
    q.play_now(songs(&["a", "b", "c"]), Some("b"));
    q.play_now(songs(&["x", "y"]), Some("y"));
    assert_eq!(item_ids(&q), ["x", "y"]);
    assert_eq!(current_id(&q).as_deref(), Some("y"));
    assert_invariants(&q);
}

#[test]
fn play_now_with_shuffle_reshuffles_every_call_with_start_song_first() {
    let mut q = Queue::with_seed(6);
    q.set_shuffle(true);
    let mut orders = HashSet::new();
    for _ in 0..10 {
        let cur = q
            .play_now(numbered(20), Some("s7"))
            .map(|it| it.song.id.clone());
        assert_eq!(cur.as_deref(), Some("s7"));
        assert_eq!(play_ids(&q)[0], "s7");
        assert_eq!(q.current_position(), Some(0));
        assert_eq!(item_ids(&q), numbered_ids(20));
        assert_invariants(&q);
        orders.insert(play_ids(&q));
    }
    assert!(orders.len() > 1, "play_now must reshuffle on every call");
}

#[test]
fn play_now_with_shuffle_and_no_start_uses_first_in_play_order() {
    let mut q = Queue::with_seed(7);
    q.set_shuffle(true);
    let cur = q.play_now(numbered(10), None).map(|it| it.uid);
    assert_eq!(cur, Some(play_uids(&q)[0]));
    assert_eq!(q.current_position(), Some(0));
}

#[test]
fn shuffle_on_puts_current_first() {
    let mut q = Queue::with_seed(8);
    q.play_now(numbered(15), Some("s9"));
    let uid = q.current().map(|it| it.uid);
    q.set_shuffle(true);
    assert!(q.shuffle());
    assert_eq!(q.current().map(|it| it.uid), uid);
    assert_eq!(q.current_position(), Some(0));
    assert_eq!(item_ids(&q), numbered_ids(15));
    assert_invariants(&q);
}

#[test]
fn shuffle_on_without_current_shuffles_everything() {
    let mut q = Queue::with_seed(9);
    q.add_last(numbered(30));
    q.set_shuffle(true);
    assert!(q.current().is_none());
    assert_ne!(play_ids(&q), numbered_ids(30));
    assert_invariants(&q);
}

#[test]
fn shuffle_off_restores_original_order_and_keeps_current() {
    let mut q = Queue::with_seed(10);
    q.set_shuffle(true);
    q.play_now(numbered(12), Some("s3"));
    q.next(Advance::Manual);
    q.next(Advance::Manual);
    let cur = q.current().map(|it| it.uid);
    let cur_id = current_id(&q);
    assert_ne!(play_ids(&q), numbered_ids(12));

    q.set_shuffle(false);
    assert!(!q.shuffle());
    assert_eq!(play_ids(&q), numbered_ids(12));
    assert_eq!(q.current().map(|it| it.uid), cur);
    let expected_pos = numbered_ids(12)
        .iter()
        .position(|id| Some(id) == cur_id.as_ref());
    assert_eq!(q.current_position(), expected_pos);
    assert_invariants(&q);
}

#[test]
fn set_shuffle_is_idempotent() {
    let mut q = Queue::with_seed(11);
    q.play_now(numbered(10), Some("s4"));
    q.set_shuffle(true);
    let order = play_uids(&q);
    q.set_shuffle(true);
    assert_eq!(play_uids(&q), order);
    q.set_shuffle(false);
    q.set_shuffle(false);
    assert_eq!(play_ids(&q), numbered_ids(10));
}

#[test]
fn toggling_shuffle_on_again_reshuffles_with_current_first() {
    let mut q = Queue::with_seed(12);
    q.play_now(numbered(20), Some("s5"));
    let mut orders = HashSet::new();
    for _ in 0..5 {
        q.set_shuffle(true);
        assert_eq!(current_id(&q).as_deref(), Some("s5"));
        assert_eq!(q.current_position(), Some(0));
        orders.insert(play_ids(&q));
        q.set_shuffle(false);
    }
    assert!(orders.len() > 1);
}

fn at(ids: &[&str], current: &str, repeat: RepeatMode) -> Queue {
    let mut q = Queue::with_seed(13);
    q.play_now(songs(ids), Some(current));
    q.set_repeat(repeat);
    q
}

#[test]
fn next_with_repeat_none() {
    for advance in [Advance::Auto, Advance::Manual] {
        let mut q = at(&["a", "b", "c"], "a", RepeatMode::None);
        assert_eq!(q.next(advance).map(|it| it.song.id.as_str()), Some("b"));
        assert_eq!(q.next(advance).map(|it| it.song.id.as_str()), Some("c"));
        assert!(q.next(advance).is_none());
        assert_eq!(current_id(&q).as_deref(), Some("a"));
    }
}

#[test]
fn next_with_repeat_all_wraps() {
    for advance in [Advance::Auto, Advance::Manual] {
        let mut q = at(&["a", "b", "c"], "b", RepeatMode::All);
        assert_eq!(q.next(advance).map(|it| it.song.id.as_str()), Some("c"));
        assert_eq!(q.next(advance).map(|it| it.song.id.as_str()), Some("a"));
        assert_eq!(current_id(&q).as_deref(), Some("a"));
    }
}

#[test]
fn next_with_repeat_one_repeats_only_on_auto() {
    let mut q = at(&["a", "b", "c"], "b", RepeatMode::One);
    let uid = q.current().map(|it| it.uid);
    assert_eq!(q.next(Advance::Auto).map(|it| it.uid), uid);
    assert_eq!(q.next(Advance::Auto).map(|it| it.uid), uid);
    assert_eq!(
        q.next(Advance::Manual).map(|it| it.song.id.as_str()),
        Some("c")
    );
    assert_eq!(
        q.next(Advance::Manual).map(|it| it.song.id.as_str()),
        Some("c")
    );
    assert_eq!(
        q.next(Advance::Auto).map(|it| it.song.id.as_str()),
        Some("c")
    );
}

#[test]
fn previous_with_every_repeat_mode() {
    for repeat in [RepeatMode::None, RepeatMode::All, RepeatMode::One] {
        let mut q = at(&["a", "b", "c"], "c", repeat);
        assert_eq!(q.previous().map(|it| it.song.id.as_str()), Some("b"));
        assert_eq!(q.previous().map(|it| it.song.id.as_str()), Some("a"));
        let expected = if repeat == RepeatMode::All { "c" } else { "a" };
        assert_eq!(q.previous().map(|it| it.song.id.as_str()), Some(expected));
        assert_eq!(current_id(&q).as_deref(), Some(expected));
    }
}

#[test]
fn single_item_queue_navigation() {
    let mut q = at(&["a"], "a", RepeatMode::None);
    assert!(q.peek_next().is_none());
    assert!(q.next(Advance::Manual).is_none());
    assert_eq!(current_id(&q).as_deref(), Some("a"));
    q.set_repeat(RepeatMode::All);
    assert_eq!(
        q.next(Advance::Auto).map(|it| it.song.id.as_str()),
        Some("a")
    );
    assert_eq!(q.previous().map(|it| it.song.id.as_str()), Some("a"));
    q.set_repeat(RepeatMode::One);
    assert_eq!(
        q.next(Advance::Manual).map(|it| it.song.id.as_str()),
        Some("a")
    );
}

#[test]
fn next_and_previous_follow_shuffled_order() {
    let mut q = Queue::with_seed(14);
    q.set_shuffle(true);
    q.play_now(numbered(10), Some("s0"));
    let order = play_uids(&q);
    for expected in &order[1..] {
        assert_eq!(q.next(Advance::Manual).map(|it| it.uid), Some(*expected));
    }
    assert!(q.next(Advance::Auto).is_none());
    assert_eq!(q.current().map(|it| it.uid), Some(order[0]));

    q.set_repeat(RepeatMode::All);
    assert_eq!(q.previous().map(|it| it.uid), order.last().copied());
    assert_eq!(q.previous().map(|it| it.uid), Some(order[8]));
    assert_eq!(q.next(Advance::Auto).map(|it| it.uid), Some(order[9]));
    assert_eq!(q.next(Advance::Auto).map(|it| it.uid), Some(order[0]));
}

#[test]
fn peek_next_always_matches_next_auto() {
    for shuffle in [false, true] {
        for repeat in [RepeatMode::None, RepeatMode::All, RepeatMode::One] {
            let mut q = Queue::with_seed(15);
            q.set_shuffle(shuffle);
            q.play_now(numbered(6), Some("s2"));
            q.set_repeat(repeat);
            for uid in play_uids(&q) {
                q.play_uid(uid);
                let peeked = q.peek_next().cloned();
                let mut moved = q.clone();
                assert_eq!(moved.next(Advance::Auto).cloned(), peeked);
            }
        }
    }
}

#[test]
fn play_uid_selects_known_items_only() {
    let mut q = at(&["a", "b", "c"], "a", RepeatMode::None);
    let c = uid_of(&q, "c");
    assert_eq!(q.play_uid(c).map(|it| it.uid), Some(c));
    assert!(q.play_uid(9999).is_none());
    assert_eq!(current_id(&q).as_deref(), Some("c"));
}

#[test]
fn add_next_without_shuffle_inserts_after_current() {
    let mut q = at(&["a", "b", "c"], "b", RepeatMode::None);
    q.add_next(songs(&["x", "y"]));
    assert_eq!(play_ids(&q), ["a", "b", "x", "y", "c"]);
    assert_eq!(current_id(&q).as_deref(), Some("b"));
    assert_eq!(
        q.next(Advance::Manual).map(|it| it.song.id.as_str()),
        Some("x")
    );
    assert_invariants(&q);
}

#[test]
fn add_next_without_current_inserts_at_front() {
    let mut q = Queue::with_seed(16);
    q.add_last(songs(&["a", "b"]));
    q.add_next(songs(&["x"]));
    assert_eq!(play_ids(&q), ["x", "a", "b"]);
}

#[test]
fn add_next_with_shuffle_inserts_after_current_in_play_order() {
    let mut q = Queue::with_seed(17);
    q.set_shuffle(true);
    q.play_now(numbered(8), Some("s3"));
    q.next(Advance::Manual);
    q.next(Advance::Manual);
    let before = play_ids(&q);
    let pos = q.current_position().unwrap_or_default();
    let cur = current_id(&q);

    q.add_next(songs(&["x", "y", "z"]));
    let after = play_ids(&q);
    assert_eq!(current_id(&q), cur);
    assert_eq!(q.current_position(), Some(pos));
    assert_eq!(after[..=pos], before[..=pos]);
    let inserted: HashSet<&str> = after[pos + 1..pos + 4].iter().map(String::as_str).collect();
    assert_eq!(inserted, HashSet::from(["x", "y", "z"]));
    assert_eq!(after[pos + 4..], before[pos + 1..]);

    let cur_index = item_ids(&q).iter().position(|id| Some(id) == cur.as_ref());
    let original = item_ids(&q);
    let idx = cur_index.unwrap_or_default();
    assert_eq!(original[idx + 1..idx + 4], ["x", "y", "z"]);
    assert_invariants(&q);

    q.set_shuffle(false);
    assert_eq!(item_ids(&q), original);
}

#[test]
fn add_last_without_shuffle_appends() {
    let mut q = at(&["a", "b"], "a", RepeatMode::None);
    q.add_last(songs(&["x", "y"]));
    assert_eq!(play_ids(&q), ["a", "b", "x", "y"]);
    assert_eq!(current_id(&q).as_deref(), Some("a"));
    assert_invariants(&q);
}

#[test]
fn add_last_with_shuffle_appends_to_play_order() {
    let mut q = Queue::with_seed(18);
    q.set_shuffle(true);
    q.play_now(numbered(6), Some("s1"));
    let before = play_ids(&q);
    q.add_last(songs(&["x", "y", "z"]));
    let after = play_ids(&q);
    assert_eq!(after[..6], before[..]);
    let tail: HashSet<&str> = after[6..].iter().map(String::as_str).collect();
    assert_eq!(tail, HashSet::from(["x", "y", "z"]));
    assert_eq!(item_ids(&q)[6..], ["x", "y", "z"]);
    assert_eq!(current_id(&q).as_deref(), Some("s1"));
    assert_invariants(&q);
}

#[test]
fn remove_non_current_items() {
    for shuffle in [false, true] {
        let mut q = Queue::with_seed(19);
        q.set_shuffle(shuffle);
        q.play_now(songs(&["a", "b", "c", "d"]), Some("b"));
        let before: Vec<String> = play_ids(&q)
            .into_iter()
            .filter(|id| id != "a" && id != "d")
            .collect();
        let (a, d) = (uid_of(&q, "a"), uid_of(&q, "d"));
        q.remove(&[a, d, 4242]);
        assert_eq!(item_ids(&q), ["b", "c"]);
        assert_eq!(play_ids(&q), before);
        assert_eq!(current_id(&q).as_deref(), Some("b"));
        assert_invariants(&q);
    }
}

#[test]
fn remove_current_moves_to_following_item_in_play_order() {
    for shuffle in [false, true] {
        let mut q = Queue::with_seed(20);
        q.set_shuffle(shuffle);
        q.play_now(numbered(8), Some("s0"));
        q.next(Advance::Manual);
        q.next(Advance::Manual);
        let order = play_uids(&q);
        let pos = q.current_position().unwrap_or_default();
        q.remove(&[order[pos], order[pos + 1]]);
        assert_eq!(q.current().map(|it| it.uid), Some(order[pos + 2]));
        assert_eq!(q.current_position(), Some(pos));
        assert_invariants(&q);
    }
}

#[test]
fn remove_current_at_end_moves_to_first_item() {
    for shuffle in [false, true] {
        let mut q = Queue::with_seed(21);
        q.set_shuffle(shuffle);
        q.play_now(numbered(4), None);
        let order = play_uids(&q);
        q.play_uid(order[3]);
        q.remove(&[order[3], order[0]]);
        assert_eq!(q.current().map(|it| it.uid), Some(order[1]));
        assert_invariants(&q);
    }
}

#[test]
fn remove_everything_leaves_no_current() {
    for shuffle in [false, true] {
        let mut q = Queue::with_seed(22);
        q.set_shuffle(shuffle);
        q.play_now(numbered(3), None);
        let all = play_uids(&q);
        q.remove(&all);
        assert!(q.is_empty());
        assert!(q.current().is_none());
        assert_eq!(q.shuffle(), shuffle);
        assert_invariants(&q);
    }
}

#[test]
fn move_items_without_shuffle() {
    let mut q = at(&["a", "b", "c", "d", "e"], "c", RepeatMode::None);
    let (a, c, e) = (uid_of(&q, "a"), uid_of(&q, "c"), uid_of(&q, "e"));
    q.move_items(&[c, a], Some(e));
    assert_eq!(item_ids(&q), ["b", "d", "a", "c", "e"]);
    assert_eq!(current_id(&q).as_deref(), Some("c"));
    assert_eq!(q.current_position(), Some(3));

    q.move_items(&[a], None);
    assert_eq!(item_ids(&q), ["b", "d", "c", "e", "a"]);

    let b = uid_of(&q, "b");
    q.move_items(&[e, a], Some(b));
    assert_eq!(item_ids(&q), ["e", "a", "b", "d", "c"]);
    assert_invariants(&q);
}

#[test]
fn move_items_before_one_of_the_moved_items() {
    let mut q = at(&["a", "b", "c", "d", "e"], "a", RepeatMode::None);
    let (b, c) = (uid_of(&q, "b"), uid_of(&q, "c"));
    let e = uid_of(&q, "e");
    q.move_items(&[e, b], Some(b));
    assert_eq!(item_ids(&q), ["a", "b", "e", "c", "d"]);
    q.move_items(&[c], Some(c));
    assert_eq!(item_ids(&q), ["a", "b", "e", "c", "d"]);
}

#[test]
fn move_items_ignores_unknown_uids() {
    let mut q = at(&["a", "b", "c"], "a", RepeatMode::None);
    let a = uid_of(&q, "a");
    q.move_items(&[a], Some(4242));
    assert_eq!(item_ids(&q), ["a", "b", "c"]);
    q.move_items(&[4242], None);
    assert_eq!(item_ids(&q), ["a", "b", "c"]);
}

#[test]
fn move_items_with_shuffle_only_reorders_play_order() {
    let mut q = Queue::with_seed(23);
    q.set_shuffle(true);
    q.play_now(numbered(8), Some("s4"));
    let order = play_uids(&q);
    let cur = q.current().map(|it| it.uid);

    q.move_items(&[order[6], order[2]], Some(order[1]));
    let mut expected = vec![order[0], order[2], order[6], order[1]];
    expected.extend([order[3], order[4], order[5], order[7]]);
    assert_eq!(play_uids(&q), expected);
    assert_eq!(item_ids(&q), numbered_ids(8));
    assert_eq!(q.current().map(|it| it.uid), cur);
    assert_eq!(q.current_position(), Some(0));

    q.move_items(&[order[0]], None);
    assert_eq!(play_uids(&q).last().copied(), Some(order[0]));
    assert_eq!(q.current_position(), Some(7));
    assert_invariants(&q);

    q.set_shuffle(false);
    assert_eq!(play_ids(&q), numbered_ids(8));
}

#[test]
fn duplicate_songs_get_distinct_increasing_uids() {
    let mut q = Queue::with_seed(24);
    q.play_now(songs(&["a", "a", "b"]), Some("a"));
    q.add_last(songs(&["a"]));
    q.add_next(songs(&["a"]));
    let uids: Vec<Uid> = q.items().iter().map(|it| it.uid).collect();
    assert_eq!(uids.iter().collect::<HashSet<_>>().len(), 5);
    assert_eq!(current_id(&q).as_deref(), Some("a"));
    assert_eq!(q.current_position(), Some(0));

    let max = uids.iter().max().copied().unwrap_or_default();
    q.play_now(songs(&["a"]), None);
    assert!(q.items()[0].uid > max);
}

#[test]
fn removing_one_duplicate_keeps_the_other() {
    let mut q = at(&["a", "b", "a"], "a", RepeatMode::None);
    let second = q.items()[2].uid;
    q.remove(&[second]);
    assert_eq!(item_ids(&q), ["a", "b"]);
    assert_eq!(q.current().map(|it| it.uid), Some(q.items()[0].uid));
}

#[test]
fn clear_keeps_modes() {
    let mut q = Queue::with_seed(25);
    q.set_shuffle(true);
    q.set_repeat(RepeatMode::All);
    q.play_now(numbered(5), None);
    q.clear();
    assert!(q.is_empty());
    assert!(q.current().is_none());
    assert!(q.shuffle());
    assert_eq!(q.repeat(), RepeatMode::All);
    let cur = q
        .play_now(numbered(3), Some("s1"))
        .map(|it| it.song.id.clone());
    assert_eq!(cur.as_deref(), Some("s1"));
    assert_eq!(q.current_position(), Some(0));
}

#[test]
fn snapshot_round_trip() {
    for shuffle in [false, true] {
        let mut q = Queue::with_seed(26);
        q.set_shuffle(shuffle);
        q.set_repeat(RepeatMode::One);
        q.play_now(numbered(7), Some("s5"));
        q.next(Advance::Manual);
        q.add_last(songs(&["x"]));

        let snapshot = q.snapshot();
        assert_eq!(snapshot.shuffled.is_some(), shuffle);
        let json = serde_json::to_string(&snapshot).unwrap_or_default();
        let decoded: QueueSnapshot = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(decoded, snapshot);

        let mut restored = Queue::from_snapshot(decoded);
        assert_eq!(restored.snapshot(), snapshot);
        assert_eq!(play_uids(&restored), play_uids(&q));
        assert_eq!(restored.current(), q.current());
        assert_eq!(restored.repeat(), RepeatMode::One);
        assert_eq!(restored.shuffle(), shuffle);

        restored.add_last(songs(&["y"]));
        assert_invariants(&restored);
        assert!(
            restored
                .items()
                .iter()
                .all(|it| it.uid < snapshot.next_uid || it.song.id == "y")
        );
    }
}

#[test]
fn from_snapshot_repairs_inconsistent_state() {
    let items = vec![
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
    ];
    let q = Queue::from_snapshot(QueueSnapshot {
        items: items.clone(),
        shuffled: Some(vec![0, 0, 7]),
        current_uid: Some(9),
        repeat: RepeatMode::All,
        next_uid: 2,
    });
    assert_invariants(&q);
    assert!(q.shuffle());
    assert_eq!(current_id(&q).as_deref(), Some("c"));
    assert_eq!(q.current_position(), Some(0));
    assert!(q.snapshot().next_uid > 9);

    let q = Queue::from_snapshot(QueueSnapshot {
        items,
        shuffled: None,
        current_uid: Some(1234),
        repeat: RepeatMode::None,
        next_uid: 0,
    });
    assert_invariants(&q);
    assert!(q.current().is_none());
    assert_eq!(item_ids(&q), ["a", "b", "c"]);
}

#[test]
fn with_seed_is_reproducible() {
    let run = |seed| {
        let mut q = Queue::with_seed(seed);
        q.set_shuffle(true);
        q.play_now(numbered(30), Some("s3"));
        q.add_next(numbered(5));
        q.add_last(numbered(5));
        play_ids(&q)
    };
    assert_eq!(run(42), run(42));
    assert_ne!(run(42), run(43));
}

#[derive(Debug, Clone)]
enum Op {
    PlayNow(usize, Option<usize>),
    AddNext(usize),
    AddLast(usize),
    Next(bool),
    Previous,
    PlayAt(usize),
    Remove(Vec<usize>),
    Move(Vec<usize>, Option<usize>),
    Shuffle(bool),
    Repeat(u8),
    Clear,
    Restore,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0..12usize, proptest::option::of(0..12usize)).prop_map(|(n, s)| Op::PlayNow(n, s)),
        (0..4usize).prop_map(Op::AddNext),
        (0..4usize).prop_map(Op::AddLast),
        any::<bool>().prop_map(Op::Next),
        Just(Op::Previous),
        (0..16usize).prop_map(Op::PlayAt),
        proptest::collection::vec(0..16usize, 0..4).prop_map(Op::Remove),
        (
            proptest::collection::vec(0..16usize, 0..4),
            proptest::option::of(0..16usize)
        )
            .prop_map(|(m, b)| Op::Move(m, b)),
        any::<bool>().prop_map(Op::Shuffle),
        (0..3u8).prop_map(Op::Repeat),
        Just(Op::Clear),
        Just(Op::Restore),
    ]
}

fn uid_at(q: &Queue, i: usize) -> Uid {
    q.items().get(i).map_or(u64::MAX, |it| it.uid)
}

fn apply(q: &mut Queue, op: &Op) {
    match op {
        Op::PlayNow(n, start) => {
            let start = start.map(|s| format!("s{s}"));
            q.play_now(numbered(*n), start.as_deref());
        }
        Op::AddNext(n) => q.add_next(numbered(*n)),
        Op::AddLast(n) => q.add_last(numbered(*n)),
        Op::Next(manual) => {
            q.next(if *manual {
                Advance::Manual
            } else {
                Advance::Auto
            });
        }
        Op::Previous => {
            q.previous();
        }
        Op::PlayAt(i) => {
            q.play_uid(uid_at(q, *i));
        }
        Op::Remove(idx) => {
            let uids: Vec<Uid> = idx.iter().map(|&i| uid_at(q, i)).collect();
            q.remove(&uids);
        }
        Op::Move(idx, before) => {
            let uids: Vec<Uid> = idx.iter().map(|&i| uid_at(q, i)).collect();
            q.move_items(&uids, before.map(|b| uid_at(q, b)));
        }
        Op::Shuffle(on) => q.set_shuffle(*on),
        Op::Repeat(r) => q.set_repeat(match r {
            0 => RepeatMode::None,
            1 => RepeatMode::All,
            _ => RepeatMode::One,
        }),
        Op::Clear => q.clear(),
        Op::Restore => *q = Queue::from_snapshot(q.snapshot()),
    }
}

proptest! {
    #[test]
    fn random_operations_keep_invariants(seed in any::<u64>(), ops in proptest::collection::vec(op(), 0..40)) {
        let mut q = Queue::with_seed(seed);
        for op in &ops {
            let before_items: Vec<String> = item_ids(&q);
            let was_shuffled = q.shuffle();
            apply(&mut q, op);
            assert_invariants(&q);

            let peeked = q.peek_next().cloned();
            prop_assert_eq!(q.clone().next(Advance::Auto).cloned(), peeked);

            if was_shuffled && q.shuffle() && matches!(op, Op::Move(..) | Op::Next(_) | Op::Previous | Op::PlayAt(_)) {
                prop_assert_eq!(item_ids(&q), before_items);
            }
            if !q.is_empty() && matches!(op, Op::PlayNow(..)) {
                prop_assert!(q.current().is_some());
            }
        }
    }
}
