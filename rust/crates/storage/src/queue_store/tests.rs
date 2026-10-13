use std::path::{Path, PathBuf};

use feishin_core::model::{ArtistRef, Song};
use feishin_core::queue::{QueueItem, QueueSnapshot, QueueTrack, RepeatMode};

use super::compact::{CompactQueue, CompactTrack, QUEUE_FORMAT};
use super::*;
use crate::StorageError;

fn song(id: &str) -> Song {
    Song {
        id: id.into(),
        server_id: "srv".into(),
        title: format!("Song {id}"),
        artists: vec![ArtistRef {
            id: "ar".into(),
            name: "Artist".into(),
        }],
        album_artists: vec![],
        album: Some("Album".into()),
        album_id: Some("al".into()),
        duration_ms: 180_000,
        track_number: Some(1),
        disc_number: None,
        year: Some(2020),
        genres: vec!["Rock".into()],
        bitrate_kbps: Some(320),
        play_count: 3,
        user_rating: None,
        favorite: true,
        cover_art_id: Some("cov".into()),
        replay_gain: None,
        path: None,
        size_bytes: None,
        container: Some("flac".into()),
        created_at: None,
        last_played_at: None,
    }
}

fn sample() -> StoredQueue {
    StoredQueue {
        snapshot: QueueSnapshot {
            items: vec![
                QueueItem {
                    uid: 1,
                    track: QueueTrack::from(&song("a")).into(),
                },
                QueueItem {
                    uid: 2,
                    track: QueueTrack::from(&song("b")).into(),
                },
            ],
            shuffled: Some(vec![1, 0]),
            current_uid: Some(2),
            repeat: RepeatMode::All,
            next_uid: 3,
        },
        position_ms: 12_345,
    }
}

fn save_stored(data_dir: &Path, stored: &StoredQueue) {
    let encoded = encode(&stored.snapshot.snapshot_ref(), stored.position_ms).unwrap();
    save(data_dir, &encoded).unwrap();
}

fn data_dir() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let data_dir = directory.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    (directory, data_dir)
}

fn file_names(data_dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(data_dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn load_without_file_is_none() {
    let (_directory, data_dir) = data_dir();
    assert_eq!(load(&data_dir).unwrap(), None);
}

#[test]
fn round_trip_and_clear() {
    let (_directory, data_dir) = data_dir();
    let queue = sample();
    save_stored(&data_dir, &queue);
    assert_eq!(load(&data_dir).unwrap(), Some(queue));
    clear(&data_dir).unwrap();
    assert_eq!(load(&data_dir).unwrap(), None);
    assert!(file_names(&data_dir).is_empty());
    clear(&data_dir).unwrap();
}

#[test]
fn save_replaces_previous_queue_atomically() {
    let (_directory, data_dir) = data_dir();
    save_stored(&data_dir, &sample());
    let mut other = sample();
    other.snapshot.items.truncate(1);
    other.snapshot.shuffled = None;
    other.snapshot.current_uid = Some(1);
    other.position_ms = 0;
    save_stored(&data_dir, &other);
    assert_eq!(load(&data_dir).unwrap(), Some(other));
    assert_eq!(file_names(&data_dir), [PLAYBACK_FILE, QUEUE_FILE]);
}

#[test]
fn empty_queue_round_trips() {
    let (_directory, data_dir) = data_dir();
    let empty = StoredQueue {
        snapshot: QueueSnapshot {
            items: Vec::new(),
            shuffled: None,
            current_uid: None,
            repeat: RepeatMode::None,
            next_uid: 1,
        },
        position_ms: 0,
    };
    save_stored(&data_dir, &empty);
    assert_eq!(load(&data_dir).unwrap(), Some(empty));
}

#[test]
fn shared_values_are_written_once() {
    let (_directory, data_dir) = data_dir();
    let mut many = sample();
    many.snapshot.items = (0..200)
        .map(|index| QueueItem {
            uid: index + 1,
            track: QueueTrack::from(&song(&format!("s{index}"))).into(),
        })
        .collect();
    many.snapshot.shuffled = None;
    many.snapshot.next_uid = 201;
    save_stored(&data_dir, &many);
    let bytes = std::fs::read(data_dir.join(QUEUE_FILE)).unwrap();
    assert!(bytes.starts_with(b"FSQ1"));
    let artist_mentions = bytes
        .windows(6)
        .filter(|window| window == b"Artist")
        .count();
    assert_eq!(artist_mentions, 1);
    assert!(bytes.len() < 200 * 40, "{} bytes", bytes.len());
    assert_eq!(load(&data_dir).unwrap(), Some(many));
}

#[test]
fn queue_saved_as_json_still_loads_and_is_migrated() {
    let (_directory, data_dir) = data_dir();
    let legacy = serde_json::json!({
        "snapshot": {
            "items": [{ "uid": 1, "song": song("a") }, { "uid": 2, "song": song("b") }],
            "shuffled": [1, 0],
            "current_uid": 2,
            "repeat": "All",
            "next_uid": 3,
        },
        "position_ms": 12_345,
    });
    std::fs::write(data_dir.join(LEGACY_QUEUE_FILE), legacy.to_string()).unwrap();
    assert_eq!(load(&data_dir).unwrap(), Some(sample()));
    assert_eq!(file_names(&data_dir), [PLAYBACK_FILE, QUEUE_FILE]);
    assert_eq!(load(&data_dir).unwrap(), Some(sample()));
}

#[test]
fn compact_json_queue_still_loads() {
    let (_directory, data_dir) = data_dir();
    let json = serde_json::to_vec(&sample()).unwrap();
    std::fs::write(data_dir.join(LEGACY_QUEUE_FILE), json).unwrap();
    assert_eq!(load(&data_dir).unwrap(), Some(sample()));
}

#[test]
fn playback_save_moves_the_current_track_and_position() {
    let (_directory, data_dir) = data_dir();
    save_stored(&data_dir, &sample());
    let playback = PlaybackState {
        current_uid: Some(1),
        position_ms: 99,
        repeat: RepeatMode::One,
    };
    save_playback(&data_dir, &playback).unwrap();
    let loaded = load(&data_dir).unwrap().unwrap();
    assert_eq!(loaded.snapshot.current_uid, Some(1));
    assert_eq!(loaded.position_ms, 99);
    assert_eq!(loaded.snapshot.repeat, RepeatMode::One);
}

#[test]
fn playback_for_an_unknown_track_keeps_the_saved_current_track() {
    let (_directory, data_dir) = data_dir();
    save_stored(&data_dir, &sample());
    let playback = PlaybackState {
        current_uid: Some(77),
        position_ms: 99,
        repeat: RepeatMode::All,
    };
    save_playback(&data_dir, &playback).unwrap();
    assert_eq!(load(&data_dir).unwrap(), Some(sample()));
}

#[test]
fn corrupt_playback_file_is_ignored() {
    let (_directory, data_dir) = data_dir();
    save_stored(&data_dir, &sample());
    std::fs::write(data_dir.join(PLAYBACK_FILE), b"FSP1\xff\xff").unwrap();
    assert_eq!(load(&data_dir).unwrap(), Some(sample()));
}

#[test]
fn corrupt_files_are_invalid() {
    let (_directory, data_dir) = data_dir();
    std::fs::write(data_dir.join(LEGACY_QUEUE_FILE), b"{not json").unwrap();
    assert!(matches!(load(&data_dir), Err(StorageError::Invalid(_))));
    for garbage in [&b"{not json"[..], b"FSQ1\x05\x01", b"FSQ9"] {
        std::fs::write(data_dir.join(QUEUE_FILE), garbage).unwrap();
        assert!(matches!(load(&data_dir), Err(StorageError::Invalid(_))));
    }
}

#[test]
fn trailing_bytes_are_invalid() {
    let (_directory, data_dir) = data_dir();
    save_stored(&data_dir, &sample());
    let mut bytes = std::fs::read(data_dir.join(QUEUE_FILE)).unwrap();
    bytes.push(0);
    std::fs::write(data_dir.join(QUEUE_FILE), bytes).unwrap();
    assert!(matches!(load(&data_dir), Err(StorageError::Invalid(_))));
}

#[test]
fn dangling_shared_value_is_invalid() {
    let (_directory, data_dir) = data_dir();
    let dangling = CompactQueue {
        strings: vec!["srv".to_owned()],
        artist_lists: vec![Vec::<ArtistRef>::new()],
        tracks: vec![CompactTrack {
            uid: 1,
            song_id: "a".to_owned(),
            title: "A".to_owned(),
            server_id: 0,
            artists: 0,
            album: Some(4),
            album_id: None,
            cover_art_id: None,
            duration_ms: 1,
        }],
        shuffled: None::<Vec<usize>>,
        current_uid: None,
        repeat: RepeatMode::None,
        next_uid: 2,
        position_ms: 0,
    };
    let bytes = QUEUE_FORMAT.encode(&dangling).unwrap();
    std::fs::write(data_dir.join(QUEUE_FILE), bytes).unwrap();
    assert!(matches!(load(&data_dir), Err(StorageError::Invalid(_))));
}
