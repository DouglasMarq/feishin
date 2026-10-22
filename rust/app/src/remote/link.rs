use std::sync::Arc;

use feishin_core::queue::{Queue, QueueItem, QueueSnapshot, RepeatMode};
use feishin_remote::{HostMessage, RemoteFailure, RemoteQueue, StateUpdate, TransferPayload};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::transfer::with_own_thumbnail;
use crate::controller::{UiSink, UiUpdate};

#[derive(Debug)]
pub enum LinkEvent {
    Transfer(Box<TransferPayload>),
    TransferAccepted,
    Failed(RemoteFailure),
    Closed,
}

pub struct Mirror {
    sink: Arc<dyn UiSink>,
    queue: Queue,
}

impl Mirror {
    pub fn new(sink: Arc<dyn UiSink>) -> Self {
        Self {
            sink,
            queue: Queue::new(),
        }
    }

    pub fn show(&mut self, update: StateUpdate) {
        let shown = match update {
            StateUpdate::NowPlaying { track } => {
                UiUpdate::NowPlaying(track.map(|track| Box::new(with_own_thumbnail(track))))
            }
            StateUpdate::Playback {
                is_playing,
                position_ms,
                duration_ms,
            } => UiUpdate::Playback {
                is_playing,
                position_ms,
                duration_ms,
            },
            StateUpdate::Volume { volume, is_muted } => UiUpdate::Volume { volume, is_muted },
            StateUpdate::Modes {
                is_shuffled,
                repeat,
            } => UiUpdate::Modes {
                is_shuffled,
                repeat,
            },
            StateUpdate::Queue { queue } => {
                self.queue = mirrored_queue(queue);
                UiUpdate::Queue(self.queue.view())
            }
            StateUpdate::QueueCurrent { current_uid } => {
                if let Some(uid) = current_uid {
                    self.queue.play_uid(uid);
                }
                UiUpdate::Queue(self.queue.view())
            }
        };
        self.sink.send(shown);
    }
}

fn mirrored_queue(queue: RemoteQueue) -> Queue {
    Queue::from_snapshot(QueueSnapshot {
        items: queue
            .items
            .into_iter()
            .map(|item| QueueItem {
                uid: item.uid,
                track: Arc::new(with_own_thumbnail(Arc::unwrap_or_clone(item.track))),
            })
            .collect(),
        shuffled: None,
        current_uid: queue.current_uid,
        repeat: RepeatMode::None,
        next_uid: 0,
    })
}

pub fn spawn_link(
    mut incoming: mpsc::Receiver<HostMessage>,
    mut mirror: Mirror,
    events: mpsc::UnboundedSender<LinkEvent>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(message) = incoming.recv().await {
            let event = match message {
                HostMessage::State { update } => {
                    mirror.show(*update);
                    None
                }
                HostMessage::Transfer { payload } => Some(LinkEvent::Transfer(payload)),
                HostMessage::TransferAccepted => Some(LinkEvent::TransferAccepted),
                HostMessage::Failed { failure } => Some(LinkEvent::Failed(failure)),
                HostMessage::Challenge { .. }
                | HostMessage::Paired { .. }
                | HostMessage::Welcome { .. }
                | HostMessage::Denied => None,
            };
            if let Some(event) = event
                && events.send(event).is_err()
            {
                return;
            }
        }
        if events.send(LinkEvent::Closed).is_err() {
            tracing::debug!("the remote player link closed after the service stopped");
        }
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use feishin_core::queue::{QueueTrack, YouTubeTrack};
    use feishin_remote::RemoteQueue;

    use super::*;

    #[derive(Default)]
    struct Shown(Mutex<Vec<UiUpdate>>);

    impl UiSink for Shown {
        fn send(&self, update: UiUpdate) {
            self.0.lock().unwrap().push(update);
        }
    }

    fn remote_queue(current_uid: u64) -> RemoteQueue {
        let mut queue = Queue::with_seed(1);
        let tracks: Vec<QueueTrack> = (0..3)
            .map(|index| {
                QueueTrack::from(&YouTubeTrack {
                    video_id: format!("v{index}"),
                    title: format!("Song {index}"),
                    artists: Vec::new(),
                    album: None,
                    thumbnail_url: None,
                    duration_ms: 1_000,
                })
            })
            .collect();
        queue.play_tracks_now(tracks, Some(0));
        RemoteQueue {
            items: queue.items().to_vec(),
            current_uid: Some(current_uid),
        }
    }

    fn shown_thumbnail(update: &UiUpdate) -> Option<String> {
        match update {
            UiUpdate::NowPlaying(Some(track)) => track.thumbnail_url().map(str::to_owned),
            UiUpdate::Queue(view) => view
                .item_at(0)
                .and_then(|item| item.track.thumbnail_url().map(str::to_owned)),
            _ => None,
        }
    }

    #[test]
    fn given_a_mirrored_track_its_thumbnail_comes_from_the_video_id() {
        let shown = Arc::new(Shown::default());
        let mut mirror = Mirror::new(shown.clone());
        let track = QueueTrack::from(&YouTubeTrack {
            video_id: "abc".into(),
            title: "Song".into(),
            artists: Vec::new(),
            album: None,
            thumbnail_url: Some("http://127.0.0.1:8080/".into()),
            duration_ms: 1_000,
        });
        let mut queue = Queue::with_seed(1);
        queue.play_tracks_now(vec![track.clone()], Some(0));

        mirror.show(StateUpdate::NowPlaying { track: Some(track) });
        mirror.show(StateUpdate::Queue {
            queue: RemoteQueue {
                items: queue.items().to_vec(),
                current_uid: None,
            },
        });

        let thumbnails: Vec<Option<String>> = shown
            .0
            .lock()
            .unwrap()
            .iter()
            .map(shown_thumbnail)
            .collect();
        let expected = Some("https://i.ytimg.com/vi/abc/hqdefault.jpg".to_owned());
        assert_eq!(thumbnails, [expected.clone(), expected]);
    }

    #[test]
    fn the_remote_queue_shows_with_the_same_songs_and_current_song() {
        let shown = Arc::new(Shown::default());
        let mut mirror = Mirror::new(shown.clone());
        mirror.show(StateUpdate::Queue {
            queue: remote_queue(2),
        });
        mirror.show(StateUpdate::QueueCurrent {
            current_uid: Some(3),
        });
        let updates = shown.0.lock().unwrap();
        let Some(UiUpdate::Queue(first)) = updates.first() else {
            panic!("expected a queue");
        };
        let Some(UiUpdate::Queue(moved)) = updates.get(1) else {
            panic!("expected a queue");
        };
        assert_eq!(first.len(), 3);
        assert_eq!(first.current().map(|item| item.uid), Some(2));
        assert_eq!(moved.current().map(|item| item.uid), Some(3));
    }
}
