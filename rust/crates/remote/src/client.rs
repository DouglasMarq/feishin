use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::BufReader;
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::mpsc;

use crate::error::RemoteError;
use crate::frame::{MAX_HANDSHAKE_FRAME_BYTES, read_message, read_message_within, write_message};
use crate::pairing::proof_of;
use crate::protocol::{ClientMessage, DeviceInfo, HostMessage, PROTOCOL_VERSION, ServerKey};
use crate::trust::TrustStore;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const CHALLENGE_TIMEOUT: Duration = Duration::from_secs(10);
const APPROVAL_WAIT: Duration = Duration::from_secs(75);
const INCOMING_MESSAGE_CAPACITY: usize = 64;

pub struct ClientLink {
    pub host: DeviceInfo,
    pub server: Option<ServerKey>,
    pub outgoing: mpsc::UnboundedSender<ClientMessage>,
    pub incoming: mpsc::Receiver<HostMessage>,
}

pub async fn connect(
    address: SocketAddr,
    me: DeviceInfo,
    trust: Arc<dyn TrustStore>,
) -> Result<ClientLink, RemoteError> {
    let stream = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(address))
        .await
        .map_err(|_| RemoteError::TimedOut)??;
    let (read_half, mut writer) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let host = greet(&mut reader, &mut writer, me, trust.as_ref()).await?;
    let server = await_welcome(&mut reader, &host, trust.as_ref()).await?;
    Ok(ClientLink {
        host,
        server,
        outgoing: spawn_writer(writer),
        incoming: spawn_reader(reader),
    })
}

async fn greet(
    reader: &mut BufReader<OwnedReadHalf>,
    writer: &mut OwnedWriteHalf,
    me: DeviceInfo,
    trust: &dyn TrustStore,
) -> Result<DeviceInfo, RemoteError> {
    let challenge = tokio::time::timeout(CHALLENGE_TIMEOUT, read_handshake_message(reader))
        .await
        .map_err(|_| RemoteError::TimedOut)??;
    let Some(HostMessage::Challenge {
        version,
        device: host,
        nonce,
    }) = challenge
    else {
        return Err(RemoteError::Protocol("expected a challenge"));
    };
    if version != PROTOCOL_VERSION {
        return Err(RemoteError::Failed(
            crate::protocol::RemoteFailure::VersionMismatch,
        ));
    }
    let proof = trust
        .host_token(&host.id)
        .map(|token| proof_of(&token, &nonce));
    let hello = ClientMessage::Hello {
        version: PROTOCOL_VERSION,
        device: me,
        proof,
    };
    write_message(writer, &hello).await?;
    Ok(host)
}

async fn await_welcome(
    reader: &mut BufReader<OwnedReadHalf>,
    host: &DeviceInfo,
    trust: &dyn TrustStore,
) -> Result<Option<ServerKey>, RemoteError> {
    loop {
        let answer = tokio::time::timeout(APPROVAL_WAIT, read_handshake_message(reader))
            .await
            .map_err(|_| RemoteError::TimedOut)??;
        match answer {
            Some(HostMessage::Paired { token }) => trust.remember_host(&host.id, token),
            Some(HostMessage::Welcome { server }) => return Ok(server),
            Some(HostMessage::Denied) => return Err(RemoteError::Denied),
            Some(HostMessage::Failed { failure }) => return Err(RemoteError::Failed(failure)),
            Some(_) => return Err(RemoteError::Protocol("expected a welcome")),
            None => return Err(connection_closed()),
        }
    }
}

async fn read_handshake_message(
    reader: &mut BufReader<OwnedReadHalf>,
) -> Result<Option<HostMessage>, RemoteError> {
    read_message_within(reader, MAX_HANDSHAKE_FRAME_BYTES).await
}

fn spawn_writer(mut writer: OwnedWriteHalf) -> mpsc::UnboundedSender<ClientMessage> {
    let (sender, mut receiver) = mpsc::unbounded_channel::<ClientMessage>();
    tokio::spawn(async move {
        while let Some(message) = receiver.recv().await {
            if let Err(error) = write_message(&mut writer, &message).await {
                tracing::debug!("sending to a remote player failed: {error}");
                return;
            }
        }
    });
    sender
}

fn spawn_reader(mut reader: BufReader<OwnedReadHalf>) -> mpsc::Receiver<HostMessage> {
    let (sender, receiver) = mpsc::channel(INCOMING_MESSAGE_CAPACITY);
    tokio::spawn(async move {
        loop {
            match read_message::<HostMessage>(&mut reader).await {
                Ok(Some(message)) => {
                    if sender.send(message).await.is_err() {
                        return;
                    }
                }
                Ok(None) => return,
                Err(error) => {
                    tracing::debug!("reading from a remote player failed: {error}");
                    return;
                }
            }
        }
    });
    receiver
}

fn connection_closed() -> RemoteError {
    std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "the connection closed").into()
}
