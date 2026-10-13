mod command;
mod connection;
mod dispatch;
mod media_session;
mod persistence;
mod playback;
mod player_events;
mod preload;
mod publish;
mod queue_ops;
mod scrobbling;
mod server_queue;
mod settings_changes;
mod sleep;
mod ui_update;
mod volume;

use std::sync::Arc;

use feishin_api::Credential;
use feishin_core::model::Song;
use feishin_core::queue::{Queue, Uid};
use feishin_storage::queue_store::StoredQueue;
use feishin_storage::settings::Settings;
use tokio::sync::mpsc;

pub use command::{Command, SettingsSnapshot, forward_command};
pub use ui_update::{QueueEntry, UiSink, UiUpdate, artist_names};

use crate::audio::AudioOutput;
use crate::media::MediaSession;
use crate::play_queue_sync::PlayQueueSync;
use crate::scrobble::ScrobbleTracker;
use crate::session::Session;
use crate::sleep_timer::SleepTimer;
use crate::store::Store;

pub struct Deps {
    pub audio: Box<dyn AudioOutput>,
    pub media: Box<dyn MediaSession>,
    pub store: Arc<dyn Store>,
    pub session: Session,
    pub ui: Arc<dyn UiSink>,
    pub commands: mpsc::UnboundedSender<Command>,
}

pub struct Controller {
    deps: Deps,
    queue: Queue,
    settings: Settings,
    scrobble_tracker: ScrobbleTracker,
    loaded: Option<Uid>,
    loading: Option<Uid>,
    preloaded: Option<Uid>,
    playing: bool,
    position_ms: u64,
    duration_ms: u64,
    play_request: u64,
    queue_sync: PlayQueueSync,
    sleep_timer: SleepTimer,
    saved_credential: Option<Credential>,
}

fn initial_queue(stored: Option<StoredQueue>, seed: Option<u64>) -> (Queue, u64) {
    match stored {
        Some(stored) => (Queue::from_snapshot(stored.snapshot), stored.position_ms),
        None => (seed.map_or_else(Queue::new, Queue::with_seed), 0),
    }
}

impl Controller {
    pub fn new(
        deps: Deps,
        settings: Settings,
        stored: Option<StoredQueue>,
        seed: Option<u64>,
    ) -> Self {
        let (queue, position_ms) = initial_queue(stored, seed);
        let duration_ms = queue.current().map_or(0, |item| item.song.duration_ms);
        let mut controller = Self {
            deps,
            queue,
            settings,
            scrobble_tracker: ScrobbleTracker::default(),
            loaded: None,
            loading: None,
            preloaded: None,
            playing: false,
            position_ms,
            duration_ms,
            play_request: 0,
            queue_sync: PlayQueueSync::default(),
            sleep_timer: SleepTimer::default(),
            saved_credential: None,
        };
        controller.publish_all();
        controller.update_media_volume();
        controller
    }

    pub async fn run(mut self, mut commands: mpsc::UnboundedReceiver<Command>) {
        while let Some(command) = commands.recv().await {
            if !self.handle(command) {
                break;
            }
        }
    }

    #[cfg(test)]
    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    #[cfg(test)]
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    #[cfg(test)]
    pub fn sleep_status(&self) -> Option<crate::sleep_timer::SleepStatus> {
        self.sleep_timer.status()
    }

    fn current_song(&self) -> Option<Song> {
        self.queue.current().map(|item| item.song.clone())
    }

    fn current_duration_ms(&self) -> u64 {
        self.queue.current().map_or(0, |item| item.song.duration_ms)
    }
}
