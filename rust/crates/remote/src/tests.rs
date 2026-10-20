use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use feishin_core::queue::{Queue, QueueTrack, YouTubeTrack};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch};

use super::*;
use crate::frame::MAX_HANDSHAKE_FRAME_BYTES;
use crate::protocol::PROTOCOL_VERSION;

#[derive(Default)]
struct MemoryTrust {
    trusted: Mutex<HashMap<DeviceId, PairingToken>>,
    hosts: Mutex<HashMap<DeviceId, PairingToken>>,
}

impl TrustStore for MemoryTrust {
    fn trusted_token(&self, device: &DeviceId) -> Option<PairingToken> {
        self.trusted.lock().unwrap().get(device).copied()
    }
    fn trust(&self, device: &DeviceInfo, token: PairingToken) {
        self.trusted
            .lock()
            .unwrap()
            .insert(device.id.clone(), token);
    }
    fn host_token(&self, host: &DeviceId) -> Option<PairingToken> {
        self.hosts.lock().unwrap().get(host).copied()
    }
    fn remember_host(&self, host: &DeviceId, token: PairingToken) {
        self.hosts.lock().unwrap().insert(host.clone(), token);
    }
}

struct FakeDelegate {
    allow: bool,
    approvals: Mutex<usize>,
    controls: mpsc::UnboundedSender<RemoteControl>,
    adopted: Mutex<Vec<TransferPayload>>,
}

#[async_trait]
impl HostDelegate for FakeDelegate {
    async fn approve(&self, _device: DeviceInfo) -> bool {
        *self.approvals.lock().unwrap() += 1;
        self.allow
    }
    fn control(&self, _device: &DeviceInfo, control: RemoteControl) -> Result<(), RemoteFailure> {
        match control {
            RemoteControl::PlayTracks {
                server: Some(_), ..
            } => Err(RemoteFailure::DifferentServer),
            other => {
                self.controls.send(other).unwrap();
                Ok(())
            }
        }
    }
    async fn adopt(&self, payload: TransferPayload) -> Result<(), RemoteFailure> {
        self.adopted.lock().unwrap().push(payload);
        Ok(())
    }
    async fn hand_over(&self) -> Result<TransferPayload, RemoteFailure> {
        Err(RemoteFailure::NothingToTransfer)
    }
    fn server(&self) -> Option<ServerKey> {
        None
    }
}

struct Running {
    address: SocketAddr,
    delegate: Arc<FakeDelegate>,
    controls: mpsc::UnboundedReceiver<RemoteControl>,
    state: watch::Sender<HostState>,
}

fn device(id: &str) -> DeviceInfo {
    DeviceInfo {
        id: DeviceId(id.to_owned()),
        name: format!("Device {id}"),
    }
}

async fn start_host(allow: bool) -> Running {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (controls_sender, controls) = mpsc::unbounded_channel();
    let delegate = Arc::new(FakeDelegate {
        allow,
        approvals: Mutex::new(0),
        controls: controls_sender,
        adopted: Mutex::new(Vec::new()),
    });
    let (state, state_receiver) = watch::channel(HostState::default());
    let host = Host::new(
        device("host"),
        delegate.clone(),
        Arc::new(MemoryTrust::default()),
        state_receiver,
    );
    tokio::spawn(host.serve(listener));
    Running {
        address,
        delegate,
        controls,
        state,
    }
}

async fn next_message(link: &mut ClientLink) -> HostMessage {
    tokio::time::timeout(Duration::from_secs(5), link.incoming.recv())
        .await
        .unwrap()
        .unwrap()
}

async fn drain_initial_state(link: &mut ClientLink) {
    for _ in 0..5 {
        assert!(matches!(
            next_message(link).await,
            HostMessage::State { .. }
        ));
    }
}

fn track(index: usize) -> QueueTrack {
    QueueTrack::from(&YouTubeTrack {
        video_id: format!("v{index}"),
        title: format!("Song {index}"),
        artists: vec!["Artist".to_owned()],
        album: None,
        thumbnail_url: None,
        duration_ms: 200_000,
    })
}

#[tokio::test]
async fn an_approved_device_reconnects_without_asking_again() {
    let mut host = start_host(true).await;
    let trust: Arc<dyn TrustStore> = Arc::new(MemoryTrust::default());
    let mut link = connect(host.address, device("phone"), Arc::clone(&trust))
        .await
        .unwrap();
    assert_eq!(link.host.name, "Device host");
    drain_initial_state(&mut link).await;
    link.outgoing
        .send(ClientMessage::Control {
            control: RemoteControl::TogglePause,
        })
        .unwrap();
    let received = tokio::time::timeout(Duration::from_secs(5), host.controls.recv())
        .await
        .unwrap();
    assert_eq!(received, Some(RemoteControl::TogglePause));
    drop(link);
    let _again = connect(host.address, device("phone"), trust).await.unwrap();
    assert_eq!(*host.delegate.approvals.lock().unwrap(), 1);
}

#[tokio::test]
async fn an_unknown_device_claiming_a_known_id_is_asked_again() {
    let host = start_host(true).await;
    let _first = connect(
        host.address,
        device("phone"),
        Arc::new(MemoryTrust::default()),
    )
    .await
    .unwrap();
    let _impostor = connect(
        host.address,
        device("phone"),
        Arc::new(MemoryTrust::default()),
    )
    .await
    .unwrap();
    assert_eq!(*host.delegate.approvals.lock().unwrap(), 2);
}

#[tokio::test]
async fn a_denied_device_cannot_connect() {
    let host = start_host(false).await;
    let refused = connect(
        host.address,
        device("phone"),
        Arc::new(MemoryTrust::default()),
    )
    .await;
    assert!(matches!(refused, Err(RemoteError::Denied)));
}

#[tokio::test]
async fn state_changes_reach_the_controlling_device() {
    let host = start_host(true).await;
    let mut link = connect(
        host.address,
        device("phone"),
        Arc::new(MemoryTrust::default()),
    )
    .await
    .unwrap();
    drain_initial_state(&mut link).await;
    let mut queue = Queue::with_seed(1);
    queue.play_tracks_now((0..3).map(track).collect(), Some(1));
    host.state.send_modify(|state| {
        state.queue = queue.view();
        state.volume = 30;
    });
    let mut kinds = Vec::new();
    for _ in 0..2 {
        if let HostMessage::State { update } = next_message(&mut link).await {
            kinds.push(match *update {
                StateUpdate::Volume { volume, .. } => format!("volume {volume}"),
                StateUpdate::Queue { queue } => {
                    format!("queue {} at {:?}", queue.items.len(), queue.current_uid)
                }
                other => format!("{other:?}"),
            });
        }
    }
    assert_eq!(kinds, ["volume 30", "queue 3 at Some(2)"]);
}

async fn connected_device() -> (Running, ClientLink) {
    let host = start_host(true).await;
    let mut link = connect(
        host.address,
        device("phone"),
        Arc::new(MemoryTrust::default()),
    )
    .await
    .unwrap();
    drain_initial_state(&mut link).await;
    (host, link)
}

#[tokio::test]
async fn refused_controls_get_the_reason() {
    let (_host, mut link) = connected_device().await;
    let server = ServerKey {
        url: "http://navidrome.example".into(),
        username: "me".into(),
    };
    let control = RemoteControl::PlayTracks {
        tracks: vec![track(0)],
        start_index: None,
        server: Some(server),
    };
    link.outgoing
        .send(ClientMessage::Control { control })
        .unwrap();
    assert_eq!(
        next_message(&mut link).await,
        HostMessage::Failed {
            failure: RemoteFailure::DifferentServer
        }
    );
}

#[tokio::test]
async fn transfers_are_accepted_or_refused() {
    let (host, mut link) = connected_device().await;
    let mut queue = Queue::with_seed(2);
    queue.play_tracks_now(vec![track(5)], Some(0));
    let payload = TransferPayload {
        snapshot: queue.snapshot(),
        position_ms: 42_000,
        playing: true,
        server: None,
    };
    let message = ClientMessage::TransferHere {
        payload: Box::new(payload.clone()),
    };
    link.outgoing.send(message).unwrap();
    assert_eq!(next_message(&mut link).await, HostMessage::TransferAccepted);
    assert_eq!(host.delegate.adopted.lock().unwrap().as_slice(), [payload]);
    link.outgoing.send(ClientMessage::PullPlayback).unwrap();
    assert_eq!(
        next_message(&mut link).await,
        HostMessage::Failed {
            failure: RemoteFailure::NothingToTransfer
        }
    );
}

#[tokio::test]
async fn a_host_without_state_updates_still_takes_controls() {
    let mut host = start_host(true).await;
    let mut link = connect(
        host.address,
        device("phone"),
        Arc::new(MemoryTrust::default()),
    )
    .await
    .unwrap();
    drain_initial_state(&mut link).await;
    drop(host.state);
    for control in [RemoteControl::Next, RemoteControl::Previous] {
        link.outgoing
            .send(ClientMessage::Control {
                control: control.clone(),
            })
            .unwrap();
        let received = tokio::time::timeout(Duration::from_secs(5), host.controls.recv())
            .await
            .unwrap();
        assert_eq!(received, Some(control));
    }
}

#[tokio::test]
async fn given_an_oversized_hello_rejects_the_connection() {
    let host = start_host(true).await;
    let (read_half, mut writer) = TcpStream::connect(host.address).await.unwrap().into_split();
    let mut reader = BufReader::new(read_half);
    let mut challenge = String::new();
    reader.read_line(&mut challenge).await.unwrap();
    let oversized_name = "x".repeat(usize::try_from(MAX_HANDSHAKE_FRAME_BYTES).unwrap() * 2);
    let hello = ClientMessage::Hello {
        version: PROTOCOL_VERSION,
        device: DeviceInfo {
            id: DeviceId("phone".to_owned()),
            name: oversized_name,
        },
        proof: None,
    };
    let mut line = serde_json::to_vec(&hello).unwrap();
    line.push(b'\n');
    let _sent = writer.write_all(&line).await;

    let mut answer = Vec::new();
    let _closed = tokio::time::timeout(Duration::from_secs(5), reader.read_to_end(&mut answer))
        .await
        .unwrap();

    assert!(answer.is_empty(), "{}", String::from_utf8_lossy(&answer));
    assert_eq!(*host.delegate.approvals.lock().unwrap(), 0);
}
