#![cfg(test)]

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use feishin_platform::{DiscordPresence, NowPlaying, PlatformError};
use serde_json::{Value, json};

const RECEIVE_TIMEOUT: Duration = Duration::from_secs(5);
const HANDSHAKE_OPCODE: u32 = 0;
const FRAME_OPCODE: u32 = 1;
const CLOSE_OPCODE: u32 = 2;

type Frame = (u32, Value);

fn read_frame(stream: &mut UnixStream) -> Option<Frame> {
    let mut header = [0u8; 8];
    stream.read_exact(&mut header).ok()?;
    let opcode = u32::from_le_bytes(header[..4].try_into().unwrap());
    let body_length = u32::from_le_bytes(header[4..].try_into().unwrap());
    let mut body = vec![0u8; body_length as usize];
    stream.read_exact(&mut body).ok()?;
    Some((opcode, serde_json::from_slice(&body).unwrap()))
}

fn write_frame(stream: &mut UnixStream, opcode: u32, value: &Value) {
    let body = value.to_string();
    stream.write_all(&opcode.to_le_bytes()).unwrap();
    stream
        .write_all(&u32::try_from(body.len()).unwrap().to_le_bytes())
        .unwrap();
    stream.write_all(body.as_bytes()).unwrap();
}

fn point_discord_ipc_at(runtime_dir: &Path) {
    unsafe {
        for key in ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"] {
            std::env::set_var(key, runtime_dir);
        }
        std::env::remove_var("SNAP");
    }
}

fn answer_frames(mut stream: UnixStream, frame_sender: &mpsc::Sender<Frame>) {
    let Some(handshake) = read_frame(&mut stream) else {
        return;
    };
    frame_sender.send(handshake).unwrap();
    write_frame(
        &mut stream,
        FRAME_OPCODE,
        &json!({ "cmd": "DISPATCH", "evt": "READY", "data": { "v": 1 } }),
    );
    while let Some((opcode, frame)) = read_frame(&mut stream) {
        let nonce = frame.get("nonce").cloned();
        frame_sender.send((opcode, frame)).unwrap();
        if opcode == CLOSE_OPCODE {
            break;
        }
        write_frame(
            &mut stream,
            FRAME_OPCODE,
            &json!({ "cmd": "SET_ACTIVITY", "evt": null, "nonce": nonce, "data": {} }),
        );
    }
}

fn spawn_mock_discord(runtime_dir: &Path) -> (JoinHandle<()>, mpsc::Receiver<Frame>) {
    let listener = UnixListener::bind(runtime_dir.join("discord-ipc-0")).unwrap();
    let (frame_sender, frame_receiver) = mpsc::channel();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        answer_frames(stream, &frame_sender);
    });
    (server, frame_receiver)
}

fn now_playing() -> NowPlaying {
    NowPlaying {
        track_id: "t1".to_owned(),
        title: "Song".to_owned(),
        artists: vec!["Artist".to_owned()],
        album: Some("Album".to_owned()),
        duration_ms: Some(200_000),
        cover_url: Some("https://example.com/cover.jpg".to_owned()),
    }
}

fn assert_activity_frame(frames: &mpsc::Receiver<Frame>) {
    let (opcode, frame) = frames.recv_timeout(RECEIVE_TIMEOUT).unwrap();
    assert_eq!(opcode, FRAME_OPCODE);
    assert_eq!(frame["cmd"], "SET_ACTIVITY");
    let activity = &frame["args"]["activity"];
    assert_eq!(activity["type"], 2);
    assert_eq!(activity["details"], "Song");
    assert_eq!(activity["state"], "Artist");
    assert_eq!(
        activity["assets"]["large_image"],
        "https://example.com/cover.jpg"
    );
    assert_eq!(activity["assets"]["large_text"], "Album");
    let start = activity["timestamps"]["start"].as_i64().unwrap();
    let end = activity["timestamps"]["end"].as_i64().unwrap();
    assert_eq!(end - start, 200_000);
}

#[test]
fn discord_presence_over_mock_ipc() {
    let runtime_dir = tempfile::tempdir().unwrap();
    point_discord_ipc_at(runtime_dir.path());

    assert!(matches!(
        DiscordPresence::connect("123"),
        Err(PlatformError::Discord(_))
    ));

    let (server, frames) = spawn_mock_discord(runtime_dir.path());
    let mut presence = DiscordPresence::connect("123").unwrap();
    let (opcode, handshake) = frames.recv_timeout(RECEIVE_TIMEOUT).unwrap();
    assert_eq!(opcode, HANDSHAKE_OPCODE);
    assert_eq!(handshake, json!({ "v": 1, "client_id": "123" }));

    presence
        .set_activity(&now_playing(), Some(30_000), false)
        .unwrap();
    assert_activity_frame(&frames);

    presence.clear().unwrap();
    let (opcode, frame) = frames.recv_timeout(RECEIVE_TIMEOUT).unwrap();
    assert_eq!(opcode, FRAME_OPCODE);
    assert_eq!(frame["args"]["activity"], Value::Null);

    drop(presence);
    let (opcode, _) = frames.recv_timeout(RECEIVE_TIMEOUT).unwrap();
    assert_eq!(opcode, CLOSE_OPCODE);
    server.join().unwrap();
}
