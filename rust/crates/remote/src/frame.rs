use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::error::RemoteError;

pub const MAX_FRAME_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_HANDSHAKE_FRAME_BYTES: u64 = 16 * 1024;

pub async fn read_message<Message: DeserializeOwned>(
    reader: &mut (impl AsyncBufRead + Unpin),
) -> Result<Option<Message>, RemoteError> {
    read_message_within(reader, MAX_FRAME_BYTES).await
}

pub async fn read_message_within<Message: DeserializeOwned>(
    reader: &mut (impl AsyncBufRead + Unpin),
    max_bytes: u64,
) -> Result<Option<Message>, RemoteError> {
    let mut line = Vec::new();
    let read = (&mut *reader)
        .take(max_bytes + 1)
        .read_until(b'\n', &mut line)
        .await?;
    if read == 0 {
        return Ok(None);
    }
    if line.last() != Some(&b'\n') {
        return Err(RemoteError::Protocol(match read as u64 > max_bytes {
            true => "a message is too large",
            false => "the connection ended in the middle of a message",
        }));
    }
    Ok(Some(serde_json::from_slice(&line)?))
}

pub async fn write_message<Message: Serialize>(
    writer: &mut (impl AsyncWrite + Unpin),
    message: &Message,
) -> Result<(), RemoteError> {
    let mut line = serde_json::to_vec(message)?;
    line.push(b'\n');
    writer.write_all(&line).await?;
    Ok(writer.flush().await?)
}

#[cfg(test)]
mod tests {
    use tokio::io::BufReader;

    use super::*;
    use crate::protocol::ClientMessage;

    #[tokio::test]
    async fn messages_are_one_json_line_each() {
        let mut buffer = Vec::new();
        write_message(&mut buffer, &ClientMessage::PullPlayback)
            .await
            .unwrap();
        write_message(&mut buffer, &ClientMessage::PullPlayback)
            .await
            .unwrap();
        assert_eq!(buffer.iter().filter(|byte| **byte == b'\n').count(), 2);
        let mut reader = BufReader::new(buffer.as_slice());
        for _ in 0..2 {
            let message: Option<ClientMessage> = read_message(&mut reader).await.unwrap();
            assert_eq!(message, Some(ClientMessage::PullPlayback));
        }
        let end: Option<ClientMessage> = read_message(&mut reader).await.unwrap();
        assert_eq!(end, None);
    }

    #[tokio::test]
    async fn cut_and_invalid_lines_are_protocol_errors() {
        let mut cut = BufReader::new(b"{\"type\":\"pull_playback\"".as_slice());
        assert!(matches!(
            read_message::<ClientMessage>(&mut cut).await,
            Err(RemoteError::Protocol(_))
        ));
        let mut invalid = BufReader::new(b"not json\n".as_slice());
        assert!(matches!(
            read_message::<ClientMessage>(&mut invalid).await,
            Err(RemoteError::Malformed(_))
        ));
    }
}
