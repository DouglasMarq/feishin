use std::sync::Arc;

use super::support::{assert_invariants, item_ids, numbered_songs, song, songs};
use crate::model::{ArtistRef, Song};
use crate::queue::{Advance, Queue, QueueItem, QueueSnapshot, QueueTrack, RepeatMode};

fn album_song(id: &str) -> Song {
    Song {
        artists: vec![ArtistRef {
            id: "artist-1".into(),
            name: "The Band".into(),
        }],
        album: Some("Record".into()),
        album_id: Some("album-1".into()),
        cover_art_id: Some("cover-1".into()),
        ..song(id)
    }
}

fn first_two_tracks(queue: &Queue) -> (&QueueTrack, &QueueTrack) {
    let items = queue.items();
    (&items[0].track, &items[1].track)
}

fn assert_shares_strings(first: &QueueTrack, second: &QueueTrack) {
    assert!(Arc::ptr_eq(&first.artists, &second.artists));
    assert!(Arc::ptr_eq(&first.server_id, &second.server_id));
    let shared = |value: &Option<Arc<str>>, other: &Option<Arc<str>>| match (value, other) {
        (Some(value), Some(other)) => Arc::ptr_eq(value, other),
        _ => false,
    };
    assert!(shared(&first.album, &second.album));
    assert!(shared(&first.album_id, &second.album_id));
    assert!(shared(&first.cover_art_id, &second.cover_art_id));
}

#[test]
fn track_keeps_the_fields_the_player_needs() {
    let track = QueueTrack::from(&album_song("a"));
    assert_eq!(track.song_id, "a");
    assert_eq!(&*track.title, "Title a");
    assert_eq!(&*track.server_id, "server");
    assert_eq!(track.artist_names(), "The Band");
    assert_eq!(track.album.as_deref(), Some("Record"));
    assert_eq!(track.album_id.as_deref(), Some("album-1"));
    assert_eq!(track.cover_art_id.as_deref(), Some("cover-1"));
    assert_eq!(track.duration_ms, 180_000);
}

#[test]
fn repeated_artist_and_album_strings_share_one_allocation() {
    let mut queue = Queue::with_seed(3);
    queue.play_now(vec![album_song("a")], None);
    queue.add_last(vec![album_song("b")]);
    let (first, second) = first_two_tracks(&queue);
    assert_shares_strings(first, second);
}

#[test]
fn restored_tracks_are_interned_again() {
    let mut queue = Queue::with_seed(4);
    queue.play_now(vec![album_song("a"), album_song("b")], Some("b"));
    let json = serde_json::to_string(&queue.snapshot()).unwrap();
    let decoded: QueueSnapshot = serde_json::from_str(&json).unwrap();
    let restored = Queue::from_snapshot(decoded);
    let (first, second) = first_two_tracks(&restored);
    assert_shares_strings(first, second);
    assert_eq!(restored.current().map(|item| item.uid), Some(2));
}

#[test]
fn legacy_items_holding_full_songs_still_load() {
    let legacy_item = serde_json::json!({ "uid": 7, "song": album_song("old") });
    let item: QueueItem = serde_json::from_value(legacy_item).unwrap();
    assert_eq!(item.uid, 7);
    assert_eq!(*item.track, QueueTrack::from(&album_song("old")));
}

#[test]
fn optional_fields_are_left_out_of_the_saved_queue() {
    let json = serde_json::to_string(&QueueTrack::from(&song("a"))).unwrap();
    assert!(!json.contains("album"));
    assert!(!json.contains("cover_art_id"));
}

#[test]
fn revision_changes_on_structural_edits_only() {
    let mut queue = Queue::with_seed(5);
    let mut last_revision = queue.revision();
    let mut assert_bumped = |queue: &Queue, is_bumped: bool| {
        assert_eq!(queue.revision() != last_revision, is_bumped);
        last_revision = queue.revision();
    };
    queue.play_now(songs(&["a", "b", "c"]), Some("a"));
    assert_bumped(&queue, true);
    queue.next(Advance::Manual);
    queue.previous();
    queue.set_repeat(RepeatMode::All);
    assert_bumped(&queue, false);
    queue.add_next(songs(&["x"]));
    assert_bumped(&queue, true);
    queue.add_last(songs(&["y"]));
    assert_bumped(&queue, true);
    queue.set_shuffle(true);
    assert_bumped(&queue, true);
    queue.move_items(&[1], None);
    assert_bumped(&queue, true);
    queue.remove(&[2]);
    assert_bumped(&queue, true);
    queue.clear();
    assert_bumped(&queue, true);
}

#[test]
fn play_uid_finds_items_after_every_kind_of_edit() {
    let mut queue = Queue::with_seed(6);
    queue.play_now(songs(&["a", "b", "c", "d"]), Some("a"));
    queue.add_next(songs(&["x"]));
    queue.remove(&[2]);
    queue.move_items(&[4], Some(1));
    queue.add_last(songs(&["y"]));
    assert_eq!(item_ids(&queue), ["d", "a", "x", "c", "y"]);
    for item in queue.items().to_vec() {
        assert_eq!(
            queue.play_uid(item.uid).map(|played| played.uid),
            Some(item.uid)
        );
    }
    assert!(queue.play_uid(2).is_none());
    assert_invariants(&queue);
}

fn song_on_album(id: &str, album: &str) -> Song {
    Song {
        album: Some(album.into()),
        ..album_song(id)
    }
}

#[test]
fn removing_the_last_track_of_an_album_frees_its_shared_strings() {
    let mut queue = Queue::with_seed(7);
    queue.play_now(
        vec![song_on_album("a", "Kept"), song_on_album("b", "Dropped")],
        Some("a"),
    );
    let album_of = |queue: &Queue, item_index: usize| {
        Arc::downgrade(queue.items()[item_index].track.album.as_ref().unwrap())
    };
    let kept_album = album_of(&queue, 0);
    let dropped_album = album_of(&queue, 1);
    queue.remove(&[queue.items()[1].uid]);
    assert_eq!(dropped_album.strong_count(), 0);
    assert_eq!(kept_album.strong_count(), 2);
    assert_invariants(&queue);
}

#[test]
fn removing_a_track_frees_its_strings_even_after_a_view_was_published() {
    let mut queue = Queue::with_seed(7);
    queue.play_now(
        vec![song_on_album("a", "Kept"), song_on_album("b", "Dropped")],
        Some("a"),
    );
    let dropped_album = queue
        .items()
        .get(1)
        .and_then(|item| item.track.album.as_ref().map(Arc::downgrade));
    drop(queue.view());
    queue.remove(&[queue.items()[1].uid]);
    assert_eq!(dropped_album.map(|album| album.strong_count()), Some(0));
    assert_invariants(&queue);
}

#[test]
fn removing_most_of_a_long_queue_releases_its_spare_capacity() {
    let mut queue = Queue::with_seed(8);
    queue.play_now(numbered_songs(1_000), Some("s0"));
    let removed_uids: Vec<_> = queue.items().iter().skip(10).map(|item| item.uid).collect();
    queue.remove(&removed_uids);
    assert_eq!(queue.len(), 10);
    assert!(queue.items.capacity() < 100);
    assert!(queue.uid_indexes.capacity() < 100);
    assert_invariants(&queue);
}
