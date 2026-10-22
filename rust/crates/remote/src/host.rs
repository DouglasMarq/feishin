use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::io::BufReader;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore, mpsc, watch};

use crate::error::RemoteError;
use crate::frame::{MAX_HANDSHAKE_FRAME_BYTES, read_message, read_message_within, write_message};
use crate::host_state::{HostState, Outbox};
use crate::lan::is_lan_peer;
use crate::pairing::{Nonce, PairingToken, proves};
use crate::protocol::{
    ClientMessage, DeviceInfo, HostMessage, PROTOCOL_VERSION, RemoteControl, RemoteFailure,
    ServerKey, TransferPayload,
};
use crate::refusals::Refusals;
use crate::trust::TrustStore;

const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
const APPROVAL_TIMEOUT: Duration = Duration::from_secs(60);
const INCOMING_MESSAGE_CAPACITY: usize = 64;
pub(crate) const MAX_PENDING_HANDSHAKES: usize = 8;

#[async_trait]
pub trait HostDelegate: Send + Sync {
    async fn approve(&self, device: DeviceInfo, peer: IpAddr) -> bool;
    fn control(&self, device: &DeviceInfo, control: RemoteControl) -> Result<(), RemoteFailure>;
    async fn adopt(&self, payload: TransferPayload) -> Result<(), RemoteFailure>;
    async fn hand_over(&self) -> Result<TransferPayload, RemoteFailure>;
    fn server(&self) -> Option<ServerKey>;
}

pub struct Host {
    me: DeviceInfo,
    delegate: Arc<dyn HostDelegate>,
    trust: Arc<dyn TrustStore>,
    state: watch::Receiver<HostState>,
    approving: Mutex<()>,
    handshakes: Arc<Semaphore>,
    refusals: Refusals,
}

impl Host {
    pub fn new(
        me: DeviceInfo,
        delegate: Arc<dyn HostDelegate>,
        trust: Arc<dyn TrustStore>,
        state: watch::Receiver<HostState>,
    ) -> Arc<Self> {
        Arc::new(Self {
            me,
            delegate,
            trust,
            state,
            approving: Mutex::new(()),
            handshakes: Arc::new(Semaphore::new(MAX_PENDING_HANDSHAKES)),
            refusals: Refusals::default(),
        })
    }

    pub async fn serve(self: Arc<Self>, listener: TcpListener) {
        loop {
            match listener.accept().await {
                Ok((stream, peer)) => self.accept(stream, peer),
                Err(error) => {
                    tracing::warn!("the remote control listener stopped: {error}");
                    return;
                }
            }
        }
    }

    fn accept(self: &Arc<Self>, stream: TcpStream, peer: SocketAddr) {
        if !is_lan_peer(peer.ip()) {
            tracing::warn!("refused a remote control connection from outside the local network");
            return;
        }
        let Ok(handshake) = Arc::clone(&self.handshakes).try_acquire_owned() else {
            tracing::warn!("refused a remote control connection: too many are being set up");
            return;
        };
        let host = Arc::clone(self);
        tokio::spawn(async move {
            if let Err(error) = host.run_session(stream, peer.ip(), handshake).await {
                tracing::info!("a remote control session ended: {error}");
            }
        });
    }

    async fn run_session(
        &self,
        stream: TcpStream,
        peer: IpAddr,
        handshake: OwnedSemaphorePermit,
    ) -> Result<(), RemoteError> {
        let (read_half, mut writer) = stream.into_split();
        let mut reader = BufReader::new(read_half);
        let device = self.handshake(&mut reader, &mut writer, peer).await?;
        drop(handshake);
        tracing::info!("{} is controlling this player", device.name);
        let incoming = spawn_reader(reader);
        Session {
            host: self,
            device,
            writer,
            outbox: Outbox::default(),
        }
        .run(incoming)
        .await
    }

    async fn handshake(
        &self,
        reader: &mut BufReader<OwnedReadHalf>,
        writer: &mut OwnedWriteHalf,
        peer: IpAddr,
    ) -> Result<DeviceInfo, RemoteError> {
        let nonce = Nonce::random()?;
        let challenge = HostMessage::Challenge {
            version: PROTOCOL_VERSION,
            device: self.me.clone(),
            nonce,
        };
        write_message(writer, &challenge).await?;
        let hello = tokio::time::timeout(
            HELLO_TIMEOUT,
            read_message_within::<ClientMessage>(reader, MAX_HANDSHAKE_FRAME_BYTES),
        )
        .await
        .map_err(|_| RemoteError::TimedOut)??;
        let Some(ClientMessage::Hello {
            version,
            device,
            proof,
        }) = hello
        else {
            return Err(RemoteError::Protocol("expected a hello"));
        };
        let device = device.cleaned();
        if version != PROTOCOL_VERSION {
            let failure = RemoteFailure::VersionMismatch;
            write_message(writer, &HostMessage::Failed { failure }).await?;
            return Err(RemoteError::Failed(failure));
        }
        let known = self
            .trust
            .trusted_token(&device.id)
            .zip(proof)
            .is_some_and(|(token, proof)| proves(&proof, &token, &nonce));
        if !known {
            self.pair(&device, peer, writer).await?;
        }
        let server = self.delegate.server();
        write_message(writer, &HostMessage::Welcome { server }).await?;
        Ok(device)
    }

    async fn pair(
        &self,
        device: &DeviceInfo,
        peer: IpAddr,
        writer: &mut OwnedWriteHalf,
    ) -> Result<(), RemoteError> {
        if self.refusals.is_cooling_down(peer, Instant::now()) {
            tracing::info!("not asking again about {peer}, it was refused a moment ago");
            return deny(writer).await;
        }
        let Ok(_approving) = self.approving.try_lock() else {
            let failure = RemoteFailure::Busy;
            write_message(writer, &HostMessage::Failed { failure }).await?;
            return Err(RemoteError::Failed(failure));
        };
        let approved = tokio::time::timeout(
            APPROVAL_TIMEOUT,
            self.delegate.approve(device.clone(), peer),
        )
        .await
        .unwrap_or_else(|_| {
            tracing::info!("nobody approved {} in time", device.name);
            false
        });
        if !approved {
            self.refusals.remember(peer, Instant::now());
            return deny(writer).await;
        }
        let token = PairingToken::random()?;
        self.trust.trust(device, token);
        write_message(writer, &HostMessage::Paired { token }).await
    }
}

async fn deny(writer: &mut OwnedWriteHalf) -> Result<(), RemoteError> {
    write_message(writer, &HostMessage::Denied).await?;
    Err(RemoteError::Denied)
}

fn spawn_reader(mut reader: BufReader<OwnedReadHalf>) -> mpsc::Receiver<ClientMessage> {
    let (sender, receiver) = mpsc::channel(INCOMING_MESSAGE_CAPACITY);
    tokio::spawn(async move {
        loop {
            match read_message::<ClientMessage>(&mut reader).await {
                Ok(Some(message)) => {
                    if sender.send(message).await.is_err() {
                        return;
                    }
                }
                Ok(None) => return,
                Err(error) => {
                    tracing::debug!("reading from a remote control client failed: {error}");
                    return;
                }
            }
        }
    });
    receiver
}

async fn next_state(state: &mut Option<watch::Receiver<HostState>>) -> HostState {
    loop {
        let Some(receiver) = state.as_mut() else {
            return std::future::pending().await;
        };
        match receiver.changed().await {
            Ok(()) => return receiver.borrow_and_update().clone(),
            Err(_) => *state = None,
        }
    }
}

struct Session<'host> {
    host: &'host Host,
    device: DeviceInfo,
    writer: OwnedWriteHalf,
    outbox: Outbox,
}

impl Session<'_> {
    async fn run(mut self, mut incoming: mpsc::Receiver<ClientMessage>) -> Result<(), RemoteError> {
        let mut state = Some(self.host.state.clone());
        let initial = self.host.state.borrow().clone();
        self.push_state(&initial).await?;
        loop {
            tokio::select! {
                message = incoming.recv() => match message {
                    Some(message) => self.on_message(message).await?,
                    None => return Ok(()),
                },
                current = next_state(&mut state) => self.push_state(&current).await?,
            }
        }
    }

    async fn push_state(&mut self, state: &HostState) -> Result<(), RemoteError> {
        for update in self.outbox.updates(state, Instant::now()) {
            let message = HostMessage::State {
                update: Box::new(update),
            };
            write_message(&mut self.writer, &message).await?;
        }
        Ok(())
    }

    async fn on_message(&mut self, message: ClientMessage) -> Result<(), RemoteError> {
        let delegate = &self.host.delegate;
        let answer = match message {
            ClientMessage::Control { control } => delegate
                .control(&self.device, control)
                .err()
                .map(|failure| HostMessage::Failed { failure }),
            ClientMessage::TransferHere { payload } => Some(match delegate.adopt(*payload).await {
                Ok(()) => HostMessage::TransferAccepted,
                Err(failure) => HostMessage::Failed { failure },
            }),
            ClientMessage::PullPlayback => Some(match delegate.hand_over().await {
                Ok(payload) => HostMessage::Transfer {
                    payload: Box::new(payload),
                },
                Err(failure) => HostMessage::Failed { failure },
            }),
            ClientMessage::Hello { .. } => None,
        };
        match answer {
            Some(answer) => write_message(&mut self.writer, &answer).await,
            None => Ok(()),
        }
    }
}
