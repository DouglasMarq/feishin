use std::collections::HashSet;

use crate::model::Song;
use crate::queue::snapshot::is_permutation;
use crate::queue::{Queue, RepeatMode, Uid};

pub(super) fn song(id: &str) -> Song {
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

pub(super) fn songs(ids: &[&str]) -> Vec<Song> {
    ids.iter().map(|id| song(id)).collect()
}

pub(super) fn numbered_songs(count: usize) -> Vec<Song> {
    (0..count).map(|index| song(&format!("s{index}"))).collect()
}

pub(super) fn numbered_ids(count: usize) -> Vec<String> {
    (0..count).map(|index| format!("s{index}")).collect()
}

pub(super) fn play_ids(queue: &Queue) -> Vec<String> {
    queue
        .display_order()
        .iter()
        .map(|item| item.song.id.clone())
        .collect()
}

pub(super) fn item_ids(queue: &Queue) -> Vec<String> {
    queue
        .items()
        .iter()
        .map(|item| item.song.id.clone())
        .collect()
}

pub(super) fn play_uids(queue: &Queue) -> Vec<Uid> {
    queue.display_order().iter().map(|item| item.uid).collect()
}

pub(super) fn current_id(queue: &Queue) -> Option<String> {
    queue.current().map(|item| item.song.id.clone())
}

pub(super) fn current_uid(queue: &Queue) -> Option<Uid> {
    queue.current().map(|item| item.uid)
}

pub(super) fn uid_of(queue: &Queue, id: &str) -> Uid {
    queue
        .items()
        .iter()
        .find(|item| item.song.id == id)
        .map(|item| item.uid)
        .unwrap_or_else(|| panic!("no item with song id {id}"))
}

pub(super) fn queue_playing(ids: &[&str], current: &str, repeat: RepeatMode) -> Queue {
    let mut queue = Queue::with_seed(13);
    queue.play_now(songs(ids), Some(current));
    queue.set_repeat(repeat);
    queue
}

pub(super) fn assert_invariants(queue: &Queue) {
    let item_uids: HashSet<Uid> = queue.items().iter().map(|item| item.uid).collect();
    assert_eq!(item_uids.len(), queue.len(), "uids must be unique");
    let display = play_uids(queue);
    assert_eq!(display.len(), queue.len());
    assert_eq!(display.iter().copied().collect::<HashSet<_>>(), item_uids);
    assert_eq!(queue.is_empty(), display.is_empty());
    assert_current_is_consistent(queue, &item_uids, &display);
    if let Some(play_order) = queue.snapshot().shuffled {
        assert!(is_permutation(&play_order, queue.len()));
    }
    let highest_uid = queue.items().iter().map(|item| item.uid).max();
    assert!(queue.snapshot().next_uid > highest_uid.unwrap_or(0));
}

fn assert_current_is_consistent(queue: &Queue, item_uids: &HashSet<Uid>, display: &[Uid]) {
    match queue.current() {
        Some(current) => {
            assert!(item_uids.contains(&current.uid));
            let position = queue.current_position().expect("current has a position");
            assert_eq!(display[position], current.uid);
        }
        None => assert_eq!(queue.current_position(), None),
    }
}
