use std::ops::Range;

use feishin_core::model::{ServerConfig, SongListQuery};
use feishin_core::queue::{QueueItem, QueueSnapshot, QueueTrack, RepeatMode, Uid};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::pairing::{Nonce, PairingToken, Proof};

pub const PROTOCOL_VERSION: u16 = 1;
pub const SERVICE_TYPE: &str = "_feishin-remote._tcp.local.";

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: DeviceId,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ServerKey {
    pub url: String,
    pub username: String,
}

impl ServerKey {
    pub fn of(config: &ServerConfig) -> Self {
        Self {
            url: normalized_url(&config.url),
            username: config.username.trim().to_lowercase(),
        }
    }
}

fn normalized_url(url: &str) -> String {
    match Url::parse(url.trim()) {
        Ok(parsed) => parsed.as_str().trim_end_matches('/').to_owned(),
        Err(_) => url.trim().trim_end_matches('/').to_lowercase(),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Hello {
        version: u16,
        device: DeviceInfo,
        proof: Option<Proof>,
    },
    Control {
        control: RemoteControl,
    },
    TransferHere {
        payload: Box<TransferPayload>,
    },
    PullPlayback,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HostMessage {
    Challenge {
        version: u16,
        device: DeviceInfo,
        nonce: Nonce,
    },
    Paired {
        token: PairingToken,
    },
    Welcome {
        server: Option<ServerKey>,
    },
    Denied,
    State {
        update: Box<StateUpdate>,
    },
    Transfer {
        payload: Box<TransferPayload>,
    },
    TransferAccepted,
    Failed {
        failure: RemoteFailure,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackPlacement {
    Next,
    Last,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListPlacement {
    Now,
    Next,
    Last,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RemoteList {
    Library { query: SongListQuery },
    Playlist { id: String },
    Folder { id: String },
    FolderTree { id: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum RemoteControl {
    TogglePause,
    Next,
    Previous,
    Seek {
        position_ms: u64,
    },
    SetVolume {
        volume: u8,
    },
    ToggleMute,
    ToggleShuffle,
    CycleRepeat,
    PlayUid {
        uid: Uid,
    },
    Remove {
        uids: Vec<Uid>,
    },
    Move {
        uids: Vec<Uid>,
        before: Option<Uid>,
    },
    PlayTracks {
        tracks: Vec<QueueTrack>,
        start_index: Option<usize>,
        server: Option<ServerKey>,
    },
    AddTracks {
        tracks: Vec<QueueTrack>,
        placement: TrackPlacement,
        server: Option<ServerKey>,
    },
    PlayList {
        list: RemoteList,
        song_id: Option<String>,
        row: usize,
        server: ServerKey,
    },
    EnqueueList {
        list: RemoteList,
        ranges: Vec<Range<usize>>,
        placement: ListPlacement,
        server: ServerKey,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteQueue {
    pub items: Vec<QueueItem>,
    pub current_uid: Option<Uid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StateUpdate {
    NowPlaying {
        track: Option<QueueTrack>,
    },
    Playback {
        playing: bool,
        position_ms: u64,
        duration_ms: u64,
    },
    Volume {
        volume: u8,
        muted: bool,
    },
    Modes {
        shuffle: bool,
        repeat: RepeatMode,
    },
    Queue {
        queue: RemoteQueue,
    },
    QueueCurrent {
        current_uid: Option<Uid>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransferPayload {
    pub snapshot: QueueSnapshot,
    pub position_ms: u64,
    pub playing: bool,
    pub server: Option<ServerKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum RemoteFailure {
    #[error("the device is connected to a different server")]
    DifferentServer,
    #[error("the device is not connected to a server")]
    NotConnected,
    #[error("there is nothing to move")]
    NothingToTransfer,
    #[error("the device runs a different version of Feishin")]
    VersionMismatch,
    #[error("the device is answering another request")]
    Busy,
    #[error("the device stopped")]
    Stopped,
}

#[cfg(test)]
mod tests {
    use feishin_core::model::ServerType;

    use super::*;

    fn config(url: &str, username: &str) -> ServerConfig {
        ServerConfig {
            id: "local-id".into(),
            name: "Home".into(),
            server_type: ServerType::Navidrome,
            url: url.into(),
            username: username.into(),
        }
    }

    #[test]
    fn servers_match_by_normalized_address_and_user() {
        assert_eq!(
            ServerKey::of(&config("HTTPS://Music.Example.com/", "Alice")),
            ServerKey::of(&config("https://music.example.com", "alice"))
        );
        assert_ne!(
            ServerKey::of(&config("https://music.example.com/Navidrome", "alice")),
            ServerKey::of(&config("https://music.example.com/navidrome", "alice"))
        );
        assert_ne!(
            ServerKey::of(&config("https://music.example.com", "alice")),
            ServerKey::of(&config("https://music.example.com", "bob"))
        );
    }

    #[test]
    fn given_server_addresses_that_differ_only_in_form_they_share_a_key() {
        assert_eq!(
            ServerKey::of(&config(" HTTP://Navidrome.Example:80/Music/ ", "feishin")).url,
            "http://navidrome.example/Music"
        );
        assert_eq!(
            ServerKey::of(&config("navidrome.example", "feishin")).url,
            "navidrome.example"
        );
    }

    #[test]
    fn messages_survive_a_json_round_trip() {
        let messages = vec![
            ClientMessage::Control {
                control: RemoteControl::Move {
                    uids: vec![3, 4],
                    before: None,
                },
            },
            ClientMessage::Control {
                control: RemoteControl::EnqueueList {
                    list: RemoteList::Playlist { id: "p1".into() },
                    ranges: vec![0..10, 20..30],
                    placement: ListPlacement::Next,
                    server: ServerKey::of(&config("http://navidrome.example:4533", "me")),
                },
            },
            ClientMessage::PullPlayback,
        ];
        for message in messages {
            let json = serde_json::to_string(&message).unwrap();
            assert_eq!(
                serde_json::from_str::<ClientMessage>(&json).unwrap(),
                message
            );
        }
        let update = HostMessage::State {
            update: Box::new(StateUpdate::Modes {
                shuffle: true,
                repeat: RepeatMode::All,
            }),
        };
        let json = serde_json::to_string(&update).unwrap();
        assert_eq!(serde_json::from_str::<HostMessage>(&json).unwrap(), update);
        let failure = HostMessage::Failed {
            failure: RemoteFailure::DifferentServer,
        };
        assert_eq!(
            serde_json::to_string(&failure).unwrap(),
            r#"{"type":"failed","failure":"different_server"}"#
        );
    }
}
