use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use feishin_core::queue::{Queue, QueueTrack, YouTubeTrack};
use feishin_player::PlayerEvent;
use feishin_remote::{
    ClientMessage, DeviceId, DeviceInfo, Host, HostDelegate, HostMessage, HostState, PairingToken,
    RemoteControl, RemoteFailure, ServerKey, TransferPayload, TrustStore, connect,
};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot, watch};

use super::harness::{AudioCall, start_harness, test_settings, test_songs};
use crate::controller::{AdoptedQueue, Command, RemoteCommand, TrackCommand};
use crate::remote::{AppHostDelegate, Approvals};
use crate::session::Session;

fn youtube_tracks(count: usize) -> Vec<QueueTrack> {
    (0..count)
        .map(|index| {
            QueueTrack::from(&YouTubeTrack {
                video_id: format!("video{index}"),
                title: format!("Video {index}"),
                artists: vec!["Artist".into()],
                album: None,
                thumbnail_url: None,
                duration_ms: 180_000,
            })
        })
        .collect()
}

#[tokio::test]
async fn a_transferred_queue_plays_from_its_position() {
    let mut harness = start_harness(test_settings(false, false));
    let mut elsewhere = Queue::with_seed(9);
    elsewhere.play_tracks_now(youtube_tracks(3), Some(1));
    let adopted = AdoptedQueue {
        snapshot: elsewhere.snapshot(),
        position_ms: 61_000,
        playing: false,
    };
    harness
        .controller
        .handle(Command::Remote(RemoteCommand::AdoptTransfer(Box::new(
            adopted,
        ))));
    assert_eq!(
        harness.queued_ids(),
        ["yt:video0", "yt:video1", "yt:video2"]
    );
    assert_eq!(harness.current_id().as_deref(), Some("yt:video1"));
    assert!(!harness.controller.is_playing());
}

#[tokio::test]
async fn handing_over_the_queue_pauses_this_player() {
    let mut harness = start_harness(test_settings(false, false));
    harness.controller.handle(Command::PlaySongs {
        songs: test_songs(4, 200_000),
        song_id: Some("s2".into()),
    });
    let uid = harness.start_current();
    harness.player(PlayerEvent::Position {
        position_ms: 42_000,
        duration_ms: None,
    });
    harness.audio.take();
    let (reply, replied) = oneshot::channel();
    harness
        .controller
        .handle(Command::Remote(RemoteCommand::TakeTransfer(reply)));
    let payload = replied.await.unwrap().unwrap();
    assert_eq!(payload.snapshot.current_uid, Some(uid));
    assert_eq!((payload.position_ms, payload.is_playing), (42_000, true));
    assert_eq!(
        payload.server,
        harness
            .session
            .get()
            .map(|server| ServerKey::of(server.config()))
    );
    assert_eq!(harness.audio.take(), [AudioCall::Pause]);
}

#[derive(Default)]
struct MemoryTrust(std::sync::Mutex<Vec<(DeviceId, PairingToken)>>);

impl TrustStore for MemoryTrust {
    fn trusted_token(&self, device: &DeviceId) -> Option<PairingToken> {
        self.find(device)
    }
    fn trust(&self, device: &DeviceInfo, token: PairingToken) {
        self.0.lock().unwrap().push((device.id.clone(), token));
    }
    fn host_token(&self, host: &DeviceId) -> Option<PairingToken> {
        self.find(host)
    }
    fn remember_host(&self, host: &DeviceId, token: PairingToken) {
        self.0.lock().unwrap().push((host.clone(), token));
    }
}

impl MemoryTrust {
    fn find(&self, device: &DeviceId) -> Option<PairingToken> {
        let entries = self.0.lock().unwrap();
        entries
            .iter()
            .find(|(id, _)| id == device)
            .map(|(_, token)| *token)
    }
}

fn device(id: &str) -> DeviceInfo {
    DeviceInfo {
        id: DeviceId(id.into()),
        name: id.into(),
    }
}

async fn received(commands: &mut mpsc::UnboundedReceiver<Command>) -> Command {
    tokio::time::timeout(Duration::from_secs(5), commands.recv())
        .await
        .unwrap()
        .unwrap()
}

struct ApprovingDelegate(AppHostDelegate);

#[async_trait]
impl HostDelegate for ApprovingDelegate {
    async fn approve(&self, _device: DeviceInfo) -> bool {
        true
    }
    fn control(&self, device: &DeviceInfo, control: RemoteControl) -> Result<(), RemoteFailure> {
        self.0.control(device, control)
    }
    async fn adopt(&self, payload: TransferPayload) -> Result<(), RemoteFailure> {
        self.0.adopt(payload).await
    }
    async fn hand_over(&self) -> Result<TransferPayload, RemoteFailure> {
        self.0.hand_over().await
    }
    fn server(&self) -> Option<ServerKey> {
        self.0.server()
    }
}

struct ControlledPlayer {
    address: std::net::SocketAddr,
    forwarded: mpsc::UnboundedReceiver<Command>,
    local_key: Option<ServerKey>,
    _state: watch::Sender<HostState>,
}

async fn controlled_player() -> ControlledPlayer {
    let harness = start_harness(test_settings(false, false));
    let session: Session = harness.session.clone();
    let local_key = session.get().map(|server| ServerKey::of(server.config()));
    let (commands, forwarded) = mpsc::unbounded_channel();
    let delegate = Arc::new(ApprovingDelegate(AppHostDelegate {
        commands,
        session,
        approvals: Approvals::default(),
        window: slint::Weak::default(),
    }));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (state, watched) = watch::channel(HostState::default());
    let trust = Arc::new(MemoryTrust::default());
    tokio::spawn(Host::new(device("speaker"), delegate, trust, watched).serve(listener));
    ControlledPlayer {
        address,
        forwarded,
        local_key,
        _state: state,
    }
}

#[tokio::test]
async fn another_device_plays_songs_on_this_player_over_the_network() {
    let mut player = controlled_player().await;
    let link = connect(
        player.address,
        device("laptop"),
        Arc::new(MemoryTrust::default()),
    )
    .await
    .unwrap();
    assert_eq!(link.server, player.local_key);
    let control = RemoteControl::PlayTracks {
        tracks: youtube_tracks(2),
        start_index: Some(1),
        server: None,
    };
    link.outgoing
        .send(ClientMessage::Control { control })
        .unwrap();
    let Command::Tracks(TrackCommand::PlayNow {
        tracks,
        start_index,
    }) = received(&mut player.forwarded).await
    else {
        panic!("expected the tracks to play");
    };
    assert_eq!((tracks.len(), start_index), (2, Some(1)));
}

#[tokio::test]
async fn another_device_takes_the_music_from_this_player() {
    let mut player = controlled_player().await;
    let mut link = connect(
        player.address,
        device("laptop"),
        Arc::new(MemoryTrust::default()),
    )
    .await
    .unwrap();
    link.outgoing.send(ClientMessage::PullPlayback).unwrap();
    let Command::Remote(RemoteCommand::TakeTransfer(reply)) = received(&mut player.forwarded).await
    else {
        panic!("expected a hand over");
    };
    let mut queue = Queue::with_seed(4);
    queue.play_tracks_now(youtube_tracks(1), Some(0));
    let payload = TransferPayload {
        snapshot: queue.snapshot(),
        position_ms: 5_000,
        is_playing: true,
        server: player.local_key.clone(),
    };
    reply.send(Ok(payload.clone())).unwrap();
    let transferred = loop {
        let message = tokio::time::timeout(Duration::from_secs(5), link.incoming.recv())
            .await
            .unwrap()
            .unwrap();
        if let HostMessage::Transfer { payload } = message {
            break payload;
        }
    };
    assert_eq!(*transferred, payload);
}
