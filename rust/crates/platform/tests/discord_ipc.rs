use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use feishin_platform::{DiscordPresence, NowPlaying, PlatformError};
use serde_json::{Value, json};

const TIMEOUT: Duration = Duration::from_secs(5);

fn read_frame(stream: &mut UnixStream) -> Option<(u32, Value)> {
    let mut header = [0u8; 8];
    stream.read_exact(&mut header).ok()?;
    let opcode = u32::from_le_bytes(header[..4].try_into().unwrap());
    let len = u32::from_le_bytes(header[4..].try_into().unwrap());
    let mut body = vec![0u8; len as usize];
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

#[test]
fn discord_presence_over_mock_ipc() {
    let dir = tempfile::tempdir().unwrap();
    unsafe {
        for key in ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"] {
            std::env::set_var(key, dir.path());
        }
        std::env::remove_var("SNAP");
    }

    assert!(matches!(
        DiscordPresence::connect("123"),
        Err(PlatformError::Discord(_))
    ));

    let listener = UnixListener::bind(dir.path().join("discord-ipc-0")).unwrap();
    let (frames_tx, frames) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let Some(handshake) = read_frame(&mut stream) else {
            return;
        };
        frames_tx.send(handshake).unwrap();
        write_frame(
            &mut stream,
            1,
            &json!({ "cmd": "DISPATCH", "evt": "READY", "data": { "v": 1 } }),
        );
        while let Some((opcode, frame)) = read_frame(&mut stream) {
            let nonce = frame.get("nonce").cloned();
            frames_tx.send((opcode, frame)).unwrap();
            if opcode == 2 {
                break;
            }
            write_frame(
                &mut stream,
                1,
                &json!({ "cmd": "SET_ACTIVITY", "evt": null, "nonce": nonce, "data": {} }),
            );
        }
    });

    let mut presence = DiscordPresence::connect("123").unwrap();
    let (opcode, handshake) = frames.recv_timeout(TIMEOUT).unwrap();
    assert_eq!(opcode, 0);
    assert_eq!(handshake, json!({ "v": 1, "client_id": "123" }));

    presence
        .set_activity(
            &NowPlaying {
                track_id: "t1".to_owned(),
                title: "Song".to_owned(),
                artists: vec!["Artist".to_owned()],
                album: Some("Album".to_owned()),
                duration_ms: Some(200_000),
                cover_url: Some("https://example.com/cover.jpg".to_owned()),
            },
            Some(30_000),
            false,
        )
        .unwrap();
    let (opcode, frame) = frames.recv_timeout(TIMEOUT).unwrap();
    assert_eq!(opcode, 1);
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

    presence.clear().unwrap();
    let (opcode, frame) = frames.recv_timeout(TIMEOUT).unwrap();
    assert_eq!(opcode, 1);
    assert_eq!(frame["args"]["activity"], Value::Null);

    drop(presence);
    let (opcode, _) = frames.recv_timeout(TIMEOUT).unwrap();
    assert_eq!(opcode, 2);
    server.join().unwrap();
}
