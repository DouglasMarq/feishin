use std::path::{Path, PathBuf};

use feishin_core::model::{ArtistRef, Song};
use feishin_core::queue::{QueueItem, QueueSnapshot, QueueTrack, RepeatMode, YouTubeTrack};

use super::compact::{CompactQueue, CompactSource, CompactTrack, QUEUE_FORMAT};
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
    assert!(bytes.starts_with(b"FSQ2"));
    let artist_mentions = bytes
        .windows(6)
        .filter(|window| window == b"Artist")
        .count();
    assert_eq!(artist_mentions, 1);
    assert!(bytes.len() < 200 * 40, "{} bytes", bytes.len());
    assert_eq!(load(&data_dir).unwrap(), Some(many));
}

fn legacy_song(id: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id, "server_id": "srv", "title": format!("Song {id}"),
        "artists": [{ "id": "ar", "name": "Artist" }], "album_artists": [],
        "album": "Album", "album_id": "al", "duration_ms": 180_000,
        "track_number": 1, "disc_number": null, "year": 2020, "genres": ["Rock"],
        "bitrate_kbps": 320, "play_count": 3, "user_rating": null, "favorite": true,
        "cover_art_id": "cov", "replay_gain": null, "path": null, "size_bytes": null,
        "container": "flac", "created_at": null, "last_played_at": null,
    })
}

#[test]
fn queue_saved_as_json_still_loads_and_is_migrated() {
    let (_directory, data_dir) = data_dir();
    let legacy = serde_json::json!({
        "snapshot": {
            "items": [{ "uid": 1, "song": legacy_song("a") }, { "uid": 2, "song": legacy_song("b") }],
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
    std::fs::write(data_dir.join(LEGACY_QUEUE_FILE), COMPACT_JSON_QUEUE).unwrap();
    assert_eq!(load(&data_dir).unwrap(), Some(sample()));
}

const COMPACT_JSON_QUEUE: &str = r#"{
  "snapshot": {
    "items": [
      {"uid": 1, "track": {"song_id":"a","server_id":"srv","title":"Song a","artists":[{"id":"ar","name":"Artist"}],"album":"Album","album_id":"al","cover_art_id":"cov","duration_ms":180000}},
      {"uid": 2, "track": {"song_id":"b","server_id":"srv","title":"Song b","artists":[{"id":"ar","name":"Artist"}],"album":"Album","album_id":"al","cover_art_id":"cov","duration_ms":180000}}
    ],
    "shuffled": [1, 0],
    "current_uid": 2,
    "repeat": "All",
    "next_uid": 3
  },
  "position_ms": 12345
}"#;

const YOUTUBE_JSON_QUEUE: &str = r#"{
  "snapshot": {
    "items": [
      {"uid": 1, "track": {"song_id":"a","server_id":"srv","title":"Song a","artists":[{"id":"ar","name":"Artist"}],"album":"Album","album_id":"al","cover_art_id":"cov","duration_ms":180000}},
      {"uid": 2, "track": {"song_id":"b","server_id":"srv","title":"Song b","artists":[{"id":"ar","name":"Artist"}],"album":"Album","album_id":"al","cover_art_id":"cov","duration_ms":180000}},
      {"uid": 3, "track": {"song_id": "yt:abc", "server_id": "youtube", "title": "Video abc",
        "artists": [{"id": "", "name": "Singer"}], "duration_ms": 1000,
        "source": {"YouTube": {"video_id": "abc", "thumbnail_url": "https://i.ytimg.com/vi/abc/hqdefault.jpg"}}}},
      {"uid": 4, "track": {"song_id": "yt:def", "server_id": "youtube", "title": "Video def",
        "artists": [{"id": "", "name": "Singer"}], "duration_ms": 1000,
        "source": {"YouTube": {"video_id": "def"}}}}
    ],
    "shuffled": null,
    "current_uid": 2,
    "repeat": "All",
    "next_uid": 5
  },
  "position_ms": 12345
}"#;

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
    assert!(matches!(
        load(&data_dir),
        Err(StorageError::Malformed { .. })
    ));
    for garbage in [&b"{not json"[..], b"FSQ1\x05\x01", b"FSQ2\x05\x01", b"FSQ9"] {
        std::fs::write(data_dir.join(QUEUE_FILE), garbage).unwrap();
        assert!(matches!(
            load(&data_dir),
            Err(StorageError::Invalid(_) | StorageError::Malformed { .. })
        ));
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
            source: CompactSource::Server,
        }],
        shuffled: None::<Vec<usize>>,
        current_uid: None,
        repeat: RepeatMode::None,
        next_uid: 2,
        position_ms: 0,
    };
    let bytes = QUEUE_FORMAT.encode(&dangling).unwrap();
    std::fs::write(data_dir.join(QUEUE_FILE), bytes).unwrap();
    assert!(matches!(
        load(&data_dir),
        Err(StorageError::Invalid(_) | StorageError::Malformed { .. })
    ));
}

const FIRST_FORMAT_SAMPLE: &[u8] = &[
    70, 83, 81, 49, 4, 3, 115, 114, 118, 5, 65, 108, 98, 117, 109, 2, 97, 108, 3, 99, 111, 118, 1,
    1, 2, 97, 114, 6, 65, 114, 116, 105, 115, 116, 2, 1, 1, 97, 6, 83, 111, 110, 103, 32, 97, 0, 0,
    1, 1, 1, 2, 1, 3, 252, 32, 191, 2, 0, 2, 1, 98, 6, 83, 111, 110, 103, 32, 98, 0, 0, 1, 1, 1, 2,
    1, 3, 252, 32, 191, 2, 0, 1, 2, 1, 0, 1, 2, 1, 3, 251, 57, 48,
];

fn with_youtube_tracks(mut stored: StoredQueue) -> StoredQueue {
    let video = |video_id: &str, thumbnail_url: Option<String>| YouTubeTrack {
        video_id: video_id.into(),
        title: format!("Video {video_id}"),
        artists: vec!["Singer".into()],
        album: None,
        thumbnail_url,
        duration_ms: 1_000,
    };
    let tracks = [
        video(
            "abc",
            Some("https://i.ytimg.com/vi/abc/hqdefault.jpg".into()),
        ),
        video("def", None),
    ];
    for (offset, track) in tracks.iter().enumerate() {
        stored.snapshot.items.push(QueueItem {
            uid: 3 + offset as u64,
            track: QueueTrack::from(track).into(),
        });
    }
    stored.snapshot.shuffled = None;
    stored.snapshot.next_uid = 5;
    stored
}

#[test]
fn queue_saved_in_the_first_compact_format_still_loads() {
    let (_directory, data_dir) = data_dir();
    std::fs::write(data_dir.join(QUEUE_FILE), FIRST_FORMAT_SAMPLE).unwrap();
    let loaded = load(&data_dir).unwrap().unwrap();
    assert_eq!(loaded, sample());
    assert!(
        loaded
            .snapshot
            .items
            .iter()
            .all(|item| !item.track.is_external())
    );
    save_stored(&data_dir, &loaded);
    let bytes = std::fs::read(data_dir.join(QUEUE_FILE)).unwrap();
    assert!(bytes.starts_with(b"FSQ2"));
    assert_eq!(load(&data_dir).unwrap(), Some(sample()));
}

#[test]
fn youtube_tracks_round_trip_with_their_video() {
    let (_directory, data_dir) = data_dir();
    let queue = with_youtube_tracks(sample());
    save_stored(&data_dir, &queue);
    let loaded = load(&data_dir).unwrap().unwrap();
    assert_eq!(loaded, queue);
    let videos: Vec<_> = loaded
        .snapshot
        .items
        .iter()
        .filter_map(|item| {
            item.track
                .youtube_video()
                .map(|video| video.video_id.to_string())
        })
        .collect();
    assert_eq!(videos, ["abc", "def"]);
}

#[test]
fn youtube_tracks_in_a_json_queue_still_load() {
    let (_directory, data_dir) = data_dir();
    let queue = with_youtube_tracks(sample());
    std::fs::write(data_dir.join(LEGACY_QUEUE_FILE), YOUTUBE_JSON_QUEUE).unwrap();
    assert_eq!(load(&data_dir).unwrap(), Some(queue));
    let bytes = std::fs::read(data_dir.join(QUEUE_FILE)).unwrap();
    assert!(bytes.starts_with(b"FSQ2"));
}

const CURRENT_FORMAT_SAMPLE: &[u8] = &[
    70, 83, 81, 50, 5, 3, 115, 114, 118, 5, 65, 108, 98, 117, 109, 2, 97, 108, 3, 99, 111, 118, 7,
    121, 111, 117, 116, 117, 98, 101, 2, 1, 2, 97, 114, 6, 65, 114, 116, 105, 115, 116, 1, 0, 6,
    83, 105, 110, 103, 101, 114, 4, 1, 1, 97, 6, 83, 111, 110, 103, 32, 97, 0, 0, 1, 1, 1, 2, 1, 3,
    252, 32, 191, 2, 0, 0, 2, 1, 98, 6, 83, 111, 110, 103, 32, 98, 0, 0, 1, 1, 1, 2, 1, 3, 252, 32,
    191, 2, 0, 0, 3, 6, 121, 116, 58, 97, 98, 99, 9, 86, 105, 100, 101, 111, 32, 97, 98, 99, 4, 1,
    0, 0, 0, 251, 232, 3, 1, 3, 97, 98, 99, 1, 40, 104, 116, 116, 112, 115, 58, 47, 47, 105, 46,
    121, 116, 105, 109, 103, 46, 99, 111, 109, 47, 118, 105, 47, 97, 98, 99, 47, 104, 113, 100,
    101, 102, 97, 117, 108, 116, 46, 106, 112, 103, 4, 6, 121, 116, 58, 100, 101, 102, 9, 86, 105,
    100, 101, 111, 32, 100, 101, 102, 4, 1, 0, 0, 0, 251, 232, 3, 1, 3, 100, 101, 102, 0, 0, 1, 2,
    1, 5, 251, 57, 48,
];

#[test]
fn given_a_queue_file_in_the_current_format_it_loads_unchanged() {
    let (_directory, data_dir) = data_dir();
    std::fs::write(data_dir.join(QUEUE_FILE), CURRENT_FORMAT_SAMPLE).unwrap();
    assert_eq!(
        load(&data_dir).unwrap(),
        Some(with_youtube_tracks(sample()))
    );
}
