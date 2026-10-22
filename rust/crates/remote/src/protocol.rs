use std::ops::Range;

use feishin_core::model::{ServerConfig, SongListQuery};
use feishin_core::queue::{QueueItem, QueueSnapshot, QueueTrack, RepeatMode, Uid};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::pairing::{Nonce, PairingToken, Proof};

pub const PROTOCOL_VERSION: u16 = 1;
pub const SERVICE_TYPE: &str = "_feishin-remote._tcp.local.";
pub const PREFERRED_PORT: u16 = 47_390;
const MAX_DEVICE_NAME_CHARS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: DeviceId,
    pub name: String,
}

impl DeviceInfo {
    pub fn cleaned(self) -> Self {
        let printable: String = self
            .name
            .chars()
            .filter(|character| !character.is_control() && !is_bidi_control(*character))
            .collect();
        let shortened: String = printable
            .trim()
            .chars()
            .take(MAX_DEVICE_NAME_CHARS)
            .collect();
        let name = match shortened.trim_end() {
            "" => self.id.0.chars().take(MAX_DEVICE_NAME_CHARS).collect(),
            trimmed => trimmed.to_owned(),
        };
        Self { id: self.id, name }
    }
}

fn is_bidi_control(character: char) -> bool {
    matches!(character, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
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
        #[serde(rename = "playing")]
        is_playing: bool,
        position_ms: u64,
        duration_ms: u64,
    },
    Volume {
        volume: u8,
        #[serde(rename = "muted")]
        is_muted: bool,
    },
    Modes {
        #[serde(rename = "shuffle")]
        is_shuffled: bool,
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
    #[serde(rename = "playing")]
    pub is_playing: bool,
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
    #[error("the device runs a different version of Cadence")]
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

    fn named(name: &str) -> DeviceInfo {
        DeviceInfo {
            id: DeviceId("d90da1a118b14ea57b1a969e".into()),
            name: name.into(),
        }
    }

    #[test]
    fn given_a_name_with_control_characters_they_are_removed() {
        assert_eq!(
            named(" Living\u{202e}room\n\u{0007} speaker ")
                .cleaned()
                .name,
            "Livingroom speaker"
        );
    }

    #[test]
    fn given_a_long_name_it_is_cut_to_the_limit() {
        let cleaned = named(&"a".repeat(16_000)).cleaned();
        assert_eq!(cleaned.name.chars().count(), MAX_DEVICE_NAME_CHARS);
    }

    #[test]
    fn given_an_empty_name_the_id_is_shown() {
        assert_eq!(
            named("\u{202e} \t").cleaned().name,
            "d90da1a118b14ea57b1a969e"
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
                is_shuffled: true,
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

    fn keeps_its_json(text: &str) -> HostMessage {
        let message: HostMessage = serde_json::from_str(text).unwrap();
        assert_eq!(
            serde_json::to_value(&message).unwrap(),
            serde_json::from_str::<serde_json::Value>(text).unwrap()
        );
        message
    }

    fn state(update: StateUpdate) -> HostMessage {
        HostMessage::State {
            update: Box::new(update),
        }
    }

    #[test]
    fn given_state_updates_from_the_previous_release_they_keep_their_wire_names() {
        let playback = r#"{"type":"state","update":{"kind":"playback","playing":true,"position_ms":1000,"duration_ms":2000}}"#;
        let volume = r#"{"type":"state","update":{"kind":"volume","volume":40,"muted":true}}"#;
        let modes = r#"{"type":"state","update":{"kind":"modes","shuffle":true,"repeat":"All"}}"#;

        assert_eq!(
            keeps_its_json(playback),
            state(StateUpdate::Playback {
                is_playing: true,
                position_ms: 1000,
                duration_ms: 2000,
            })
        );
        assert_eq!(
            keeps_its_json(volume),
            state(StateUpdate::Volume {
                volume: 40,
                is_muted: true,
            })
        );
        assert_eq!(
            keeps_its_json(modes),
            state(StateUpdate::Modes {
                is_shuffled: true,
                repeat: RepeatMode::All,
            })
        );
    }

    #[test]
    fn given_a_transfer_and_a_favorites_list_they_keep_their_wire_names() {
        let payload = TransferPayload {
            snapshot: feishin_core::queue::Queue::new().snapshot(),
            position_ms: 0,
            is_playing: true,
            server: None,
        };
        let list = RemoteList::Library {
            query: SongListQuery {
                is_favorite: Some(true),
                ..SongListQuery::default()
            },
        };

        let payload_json = serde_json::to_value(&payload).unwrap();
        let list_json = serde_json::to_value(&list).unwrap();

        assert_eq!(payload_json["playing"], true);
        assert!(payload_json.get("is_playing").is_none());
        assert_eq!(list_json["query"]["favorite"], true);
        assert!(list_json["query"].get("is_favorite").is_none());
    }
}
