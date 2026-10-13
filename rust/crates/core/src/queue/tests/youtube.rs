use std::sync::Arc;

use super::support::{assert_invariants, item_ids, songs};
use crate::queue::{
    Queue, QueueSnapshot, QueueTrack, TrackSource, YOUTUBE_SERVER_ID, YouTubeTrack,
};

fn youtube(video_id: &str) -> YouTubeTrack {
    YouTubeTrack {
        video_id: video_id.into(),
        title: format!("Video {video_id}"),
        artists: vec!["Singer".into(), " ".into()],
        album: None,
        thumbnail_url: Some(format!("https://i.ytimg.com/vi/{video_id}/hqdefault.jpg")),
        duration_ms: 200_000,
    }
}

fn youtube_tracks(video_ids: &[&str]) -> Vec<QueueTrack> {
    video_ids
        .iter()
        .map(|video_id| QueueTrack::from(&youtube(video_id)))
        .collect()
}

#[test]
fn youtube_tracks_carry_their_video_and_no_server_ids() {
    let track = QueueTrack::from(&youtube("abc"));
    assert_eq!(track.song_id, "yt:abc");
    assert_eq!(&*track.server_id, YOUTUBE_SERVER_ID);
    assert!(track.is_external());
    assert_eq!(track.server_song_id(), None);
    assert_eq!(track.artist_names(), "Singer");
    assert_eq!(track.artists[0].id, "");
    assert_eq!(track.cover_art_id, None);
    assert_eq!(
        track.youtube_video().map(|video| &*video.video_id),
        Some("abc")
    );
    assert_eq!(
        track.thumbnail_url(),
        Some("https://i.ytimg.com/vi/abc/hqdefault.jpg")
    );
}

#[test]
fn server_songs_keep_their_server_source() {
    let track = QueueTrack::from(&super::support::song("a"));
    assert_eq!(track.source, TrackSource::Server);
    assert_eq!(track.server_song_id(), Some("a"));
    assert!(!track.is_external());
}

#[test]
fn playing_tracks_now_replaces_the_queue_and_starts_at_the_index() {
    let mut queue = Queue::with_seed(21);
    queue.play_now(songs(&["a", "b"]), Some("a"));
    let current = queue.play_tracks_now(youtube_tracks(&["x", "y", "z"]), Some(1));
    assert_eq!(
        current.map(|item| item.track.song_id.to_string()),
        Some("yt:y".into())
    );
    assert_eq!(item_ids(&queue), ["yt:x", "yt:y", "yt:z"]);
    assert!(queue.play_tracks_now(Vec::new(), Some(0)).is_none());
    assert_invariants(&queue);
}

#[test]
fn tracks_can_be_queued_next_and_last_between_songs() {
    let mut queue = Queue::with_seed(22);
    queue.play_now(songs(&["a", "b"]), Some("a"));
    queue.add_tracks_next(youtube_tracks(&["x"]));
    queue.add_tracks_last(youtube_tracks(&["y"]));
    queue.add_tracks_next(Vec::new());
    assert_eq!(item_ids(&queue), ["a", "yt:x", "b", "yt:y"]);
    assert_invariants(&queue);
}

#[test]
fn shuffled_queues_take_tracks_too() {
    let mut queue = Queue::with_seed(23);
    queue.play_now(songs(&["a", "b", "c"]), Some("a"));
    queue.set_shuffle(true);
    queue.add_tracks_next(youtube_tracks(&["x"]));
    queue.add_tracks_last(youtube_tracks(&["y"]));
    assert_eq!(queue.len(), 5);
    assert_invariants(&queue);
}

#[test]
fn queued_tracks_share_strings_with_songs() {
    let mut queue = Queue::with_seed(24);
    queue.play_tracks_now(youtube_tracks(&["x", "y"]), None);
    let items = queue.items();
    assert!(Arc::ptr_eq(
        &items[0].track.server_id,
        &items[1].track.server_id
    ));
}

#[test]
fn youtube_tracks_survive_a_json_snapshot() {
    let mut queue = Queue::with_seed(25);
    queue.play_tracks_now(youtube_tracks(&["x"]), None);
    queue.add_last(songs(&["a"]));
    let json = serde_json::to_string(&queue.snapshot()).unwrap();
    let restored = Queue::from_snapshot(serde_json::from_str::<QueueSnapshot>(&json).unwrap());
    assert_eq!(restored.items(), queue.items());
}

#[test]
fn server_tracks_leave_the_source_out_of_json() {
    let json = serde_json::to_string(&QueueTrack::from(&super::support::song("a"))).unwrap();
    assert!(!json.contains("source"));
    let track: QueueTrack = serde_json::from_str(&json).unwrap();
    assert_eq!(track.source, TrackSource::Server);
}
