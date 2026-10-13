use std::ops::Range;

use feishin_core::model::{ServerConfig, ServerPlayQueue, Song};
use feishin_core::queue::Uid;
use feishin_platform::MediaControlEvent;
use feishin_player::{AudioDevice, PlayerEvent};
use feishin_storage::settings::{IntegrationSettings, PlaybackSettings, Settings, UiSettings};
use tokio::sync::{mpsc, oneshot};

use crate::list_stream::{ListRow, Placement, SongSource, StreamMessage};
use crate::sleep_timer::SleepMode;

pub enum Command {
    Player(PlayerEvent),
    Media(MediaControlEvent),
    Connected {
        config: ServerConfig,
        others: Vec<ServerConfig>,
    },
    PlayFromList {
        source: SongSource,
        song_id: Option<String>,
        row_index: ListRow,
    },
    EnqueueFromList {
        source: SongSource,
        ranges: Vec<Range<usize>>,
        placement: Placement,
    },
    ListStream(StreamMessage),
    TogglePause,
    Next,
    Previous,
    Seek(u64),
    SetVolume(u8),
    ToggleMute,
    ToggleShuffle,
    CycleRepeat,
    PlayUid(Uid),
    PlaySongs {
        songs: Vec<Song>,
        song_id: Option<String>,
    },
    AddNext(Vec<Song>),
    AddLast(Vec<Song>),
    Remove(Vec<Uid>),
    Move {
        uids: Vec<Uid>,
        before: Option<Uid>,
    },
    SleepTimer(Option<SleepMode>),
    SleepTick {
        elapsed_ms: u64,
    },
    RestoreQueue {
        request: u64,
        queue: ServerPlayQueue,
    },
    Shutdown {
        ui: UiSettings,
        done: oneshot::Sender<()>,
    },
    GetSettings(oneshot::Sender<SettingsSnapshot>),
    ApplyPlayback(PlaybackSettings),
    SaveIntegrations(IntegrationSettings),
    SaveInterface {
        queue_panel_open: bool,
        follow_current_song: bool,
    },
    RemoveServer(String),
}

pub fn forward_command(commands: &mpsc::UnboundedSender<Command>, command: Command) {
    if commands.send(command).is_err() {
        tracing::debug!("the controller has stopped, dropping a command");
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SettingsSnapshot {
    pub settings: Settings,
    pub devices: Vec<AudioDevice>,
}
