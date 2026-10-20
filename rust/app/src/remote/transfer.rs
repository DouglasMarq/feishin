use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_core::queue::{QueueItem, QueueSnapshot, QueueTrack};
use feishin_remote::{RemoteFailure, ServerKey, TransferPayload};

use crate::controller::AdoptedQueue;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalServer {
    pub key: ServerKey,
    pub id: Arc<str>,
}

impl LocalServer {
    pub fn of(server: &dyn MusicServer) -> Self {
        Self {
            key: ServerKey::of(server.config()),
            id: Arc::from(server.config().id.as_str()),
        }
    }
}

struct TrackAdapter<'local> {
    local: Option<&'local LocalServer>,
    is_same_server: bool,
}

impl<'local> TrackAdapter<'local> {
    fn new(from: Option<&ServerKey>, local: Option<&'local LocalServer>) -> Self {
        let is_same_server = from
            .zip(local)
            .is_some_and(|(from, local)| *from == local.key);
        Self {
            local,
            is_same_server,
        }
    }

    fn adapt(&self, track: QueueTrack) -> Option<QueueTrack> {
        match (track.source.is_server(), self.is_same_server, self.local) {
            (false, _, _) => Some(track),
            (true, true, Some(local)) => Some(QueueTrack {
                server_id: Arc::clone(&local.id),
                ..track
            }),
            (true, _, _) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AdaptedTracks {
    pub tracks: Vec<QueueTrack>,
    pub start_index: Option<usize>,
}

pub fn adapt_tracks(
    tracks: Vec<QueueTrack>,
    start_index: Option<usize>,
    from: Option<&ServerKey>,
    local: Option<&LocalServer>,
) -> Result<AdaptedTracks, RemoteFailure> {
    let adapter = TrackAdapter::new(from, local);
    let mut kept = Vec::with_capacity(tracks.len());
    let mut kept_start = None;
    for (index, track) in tracks.into_iter().enumerate() {
        if let Some(adapted) = adapter.adapt(track) {
            if Some(index) == start_index {
                kept_start = Some(kept.len());
            }
            kept.push(adapted);
        }
    }
    match kept.is_empty() {
        true => Err(RemoteFailure::DifferentServer),
        false => Ok(AdaptedTracks {
            tracks: kept,
            start_index: kept_start,
        }),
    }
}

pub fn adapt_payload(
    payload: TransferPayload,
    local: Option<&LocalServer>,
) -> Result<AdoptedQueue, RemoteFailure> {
    let adapter = TrackAdapter::new(payload.server.as_ref(), local);
    let QueueSnapshot {
        items,
        shuffled,
        current_uid,
        repeat,
        next_uid,
    } = payload.snapshot;
    let original_count = items.len();
    let kept: Vec<QueueItem> = items
        .into_iter()
        .filter_map(|item| {
            let track = adapter.adapt(Arc::unwrap_or_clone(item.track))?;
            Some(QueueItem {
                uid: item.uid,
                track: Arc::new(track),
            })
        })
        .collect();
    if kept.is_empty() {
        return Err(match original_count {
            0 => RemoteFailure::NothingToTransfer,
            _ => RemoteFailure::DifferentServer,
        });
    }
    let current_kept = kept.iter().any(|item| Some(item.uid) == current_uid);
    let is_everything_kept = kept.len() == original_count;
    Ok(AdoptedQueue {
        snapshot: QueueSnapshot {
            items: kept,
            shuffled: shuffled.filter(|_| is_everything_kept),
            current_uid: current_uid.filter(|_| current_kept),
            repeat,
            next_uid,
        },
        position_ms: if current_kept { payload.position_ms } else { 0 },
        is_playing: payload.is_playing && current_kept,
    })
}

#[cfg(test)]
mod tests {
    use feishin_core::model::Song;
    use feishin_core::queue::{Queue, RepeatMode, YouTubeTrack};

    use super::*;
    use crate::demo::test_song;

    fn key(url: &str) -> ServerKey {
        ServerKey {
            url: url.into(),
            username: "me".into(),
        }
    }

    fn local() -> LocalServer {
        LocalServer {
            key: key("http://navidrome.example"),
            id: Arc::from("local-id"),
        }
    }

    fn server_track(id: &str) -> QueueTrack {
        let song = Song {
            server_id: "remote-id".into(),
            ..test_song(id, 200_000)
        };
        QueueTrack::from(&song)
    }

    fn youtube_track(id: &str) -> QueueTrack {
        QueueTrack::from(&YouTubeTrack {
            video_id: id.into(),
            title: id.into(),
            artists: Vec::new(),
            album: None,
            thumbnail_url: None,
            duration_ms: 1_000,
        })
    }

    #[test]
    fn tracks_from_the_same_server_are_moved_to_the_local_server_id() {
        let tracks = vec![server_track("a"), youtube_track("y")];
        let adapted = adapt_tracks(
            tracks,
            Some(1),
            Some(&key("http://navidrome.example")),
            Some(&local()),
        )
        .unwrap();
        assert_eq!(adapted.tracks.len(), 2);
        assert_eq!(&*adapted.tracks[0].server_id, "local-id");
        assert_eq!(adapted.start_index, Some(1));
    }

    #[test]
    fn tracks_from_another_server_are_dropped_but_youtube_songs_stay() {
        let tracks = vec![server_track("a"), youtube_track("y"), server_track("b")];
        let adapted =
            adapt_tracks(tracks, Some(1), Some(&key("http://other")), Some(&local())).unwrap();
        assert_eq!(adapted.tracks.len(), 1);
        assert_eq!(adapted.start_index, Some(0));
        let only_server = vec![server_track("a")];
        assert_eq!(
            adapt_tracks(only_server, None, None, Some(&local())),
            Err(RemoteFailure::DifferentServer)
        );
    }

    fn payload(tracks: Vec<QueueTrack>, server: &str) -> TransferPayload {
        let mut queue = Queue::with_seed(3);
        queue.play_tracks_now(tracks, Some(1));
        queue.set_repeat(RepeatMode::All);
        TransferPayload {
            snapshot: queue.snapshot(),
            position_ms: 30_000,
            is_playing: true,
            server: Some(key(server)),
        }
    }

    #[test]
    fn a_transfer_from_the_same_server_keeps_the_position_and_the_current_song() {
        let adopted = adapt_payload(
            payload(
                vec![server_track("a"), server_track("b")],
                "http://navidrome.example",
            ),
            Some(&local()),
        )
        .unwrap();
        assert_eq!(adopted.snapshot.items.len(), 2);
        assert_eq!(adopted.snapshot.current_uid, Some(2));
        assert_eq!(adopted.snapshot.repeat, RepeatMode::All);
        assert_eq!((adopted.position_ms, adopted.is_playing), (30_000, true));
    }

    #[test]
    fn a_transfer_that_loses_the_current_song_starts_over_paused() {
        let adopted = adapt_payload(
            payload(vec![youtube_track("y"), server_track("b")], "http://other"),
            Some(&local()),
        )
        .unwrap();
        assert_eq!(adopted.snapshot.items.len(), 1);
        assert_eq!(adopted.snapshot.current_uid, None);
        assert_eq!((adopted.position_ms, adopted.is_playing), (0, false));
        assert_eq!(
            adapt_payload(payload(vec![server_track("a")], "http://other"), None),
            Err(RemoteFailure::DifferentServer)
        );
    }
}
