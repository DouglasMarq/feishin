mod bus_name;
mod conversions;
mod metadata;
mod player_core;
mod player_interface;
mod root_interface;
mod service;
mod snapshot;

use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use mpris_server::Property;
use tokio::sync::mpsc;

use crate::{MediaControlEvent, NowPlaying, PlatformError, PlaybackStatus, Result};
use bus_name::bus_suffix;
use player_core::MprisPlayerCore;
use snapshot::PlaybackSnapshot;

const STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
const MPRIS_THREAD_NAME: &str = "mpris";

type EventCallback = Box<dyn Fn(MediaControlEvent) + Send + Sync>;

enum Update {
    Metadata(NowPlaying),
    Status(PlaybackStatus),
    Volume(f64),
    Emit(Vec<Property>),
}

#[derive(Clone)]
struct MprisPlayer {
    core: Arc<MprisPlayerCore>,
}

pub struct MediaControls {
    updates: mpsc::UnboundedSender<Update>,
}

impl MediaControls {
    pub fn new(
        app_name: &str,
        on_event: Box<dyn Fn(MediaControlEvent) + Send + Sync>,
    ) -> Result<Self> {
        let (updates, update_receiver) = mpsc::unbounded_channel();
        let (ready_sender, ready_receiver) = std_mpsc::sync_channel(1);
        let player = MprisPlayer {
            core: Arc::new(MprisPlayerCore {
                identity: app_name.to_owned(),
                bus_suffix: bus_suffix(app_name),
                snapshot: Mutex::new(PlaybackSnapshot::new()),
                on_event,
                updates: updates.downgrade(),
            }),
        };
        thread::Builder::new()
            .name(MPRIS_THREAD_NAME.to_owned())
            .spawn(move || service::run(player, update_receiver, ready_sender))
            .map_err(|error| PlatformError::MediaControls(error.to_string()))?;
        match ready_receiver.recv_timeout(STARTUP_TIMEOUT) {
            Ok(registered) => registered.map(|()| Self { updates }),
            Err(_) => Err(PlatformError::MediaControls(
                "timed out registering on the session bus".to_owned(),
            )),
        }
    }

    pub fn set_metadata(&mut self, now_playing: &NowPlaying) -> Result<()> {
        self.send(Update::Metadata(now_playing.clone()))
    }

    pub fn set_status(&mut self, status: PlaybackStatus) -> Result<()> {
        self.send(Update::Status(status))
    }

    pub fn set_volume(&mut self, volume: f64) -> Result<()> {
        self.send(Update::Volume(volume))
    }

    fn send(&self, update: Update) -> Result<()> {
        self.updates
            .send(update)
            .map_err(|_| PlatformError::MediaControls("MPRIS thread is not running".to_owned()))
    }
}
