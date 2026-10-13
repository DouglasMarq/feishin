#![cfg(test)]

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use feishin_player::{Player, PlayerConfig, PlayerEvent};

const METADATA_INTERVAL: usize = 4096;
const SAMPLE_RATE: u32 = 16_000;
const STREAMED_SECONDS: usize = 20;
const WAIT_TIMEOUT: Duration = Duration::from_secs(15);
const STREAM_TITLE: &str = "Test Artist - Live Song";

fn wav_header() -> Vec<u8> {
    let byte_rate = SAMPLE_RATE * 2;
    let mut header = Vec::with_capacity(44);
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&u32::MAX.to_le_bytes());
    header.extend_from_slice(b"WAVEfmt ");
    header.extend_from_slice(&16_u32.to_le_bytes());
    header.extend_from_slice(&1_u16.to_le_bytes());
    header.extend_from_slice(&1_u16.to_le_bytes());
    header.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    header.extend_from_slice(&byte_rate.to_le_bytes());
    header.extend_from_slice(&2_u16.to_le_bytes());
    header.extend_from_slice(&16_u16.to_le_bytes());
    header.extend_from_slice(b"data");
    header.extend_from_slice(&(u32::MAX - 36).to_le_bytes());
    header
}

fn metadata_block(title: Option<&str>) -> Vec<u8> {
    let Some(title) = title else {
        return vec![0];
    };
    let mut text = format!("StreamTitle='{title}';").into_bytes();
    let blocks = text.len().div_ceil(16);
    text.resize(blocks * 16, 0);
    let mut block = vec![u8::try_from(blocks).unwrap()];
    block.extend(text);
    block
}

fn audio_bytes() -> Vec<u8> {
    let mut audio = wav_header();
    audio.resize(audio.len() + SAMPLE_RATE as usize * 2 * STREAMED_SECONDS, 0);
    audio
}

fn read_request(stream: &TcpStream) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    while reader.read_line(&mut line).is_ok_and(|read| read > 2) {
        line.clear();
    }
}

fn serve_icy_stream(mut stream: TcpStream) {
    read_request(&stream);
    let head = format!(
        "HTTP/1.0 200 OK\r\nContent-Type: audio/wav\r\nicy-name: Test Radio\r\nicy-metaint: {METADATA_INTERVAL}\r\n\r\n"
    );
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    for (index, chunk) in audio_bytes().chunks(METADATA_INTERVAL).enumerate() {
        let title = (index == 0).then_some(STREAM_TITLE);
        let sent = stream
            .write_all(chunk)
            .and_then(|()| stream.write_all(&metadata_block(title)));
        if sent.is_err() {
            return;
        }
    }
}

fn start_icy_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            thread::spawn(move || serve_icy_stream(stream));
        }
    });
    format!("http://{address}/stream")
}

fn player() -> (Player, Receiver<PlayerEvent>) {
    let (sender, receiver) = mpsc::channel();
    let player = Player::with_null_output(
        PlayerConfig::default(),
        Box::new(move |event| {
            sender.send(event).ok();
        }),
    )
    .unwrap();
    (player, receiver)
}

fn wait_for_title(events: &Receiver<PlayerEvent>) -> Vec<PlayerEvent> {
    let deadline = Instant::now() + WAIT_TIMEOUT;
    let mut seen = Vec::new();
    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        let Ok(event) = events.recv_timeout(remaining) else {
            break;
        };
        let is_title = event == PlayerEvent::StreamTitle(Some(STREAM_TITLE.into()));
        seen.push(event);
        if is_title {
            break;
        }
    }
    seen
}

#[test]
fn live_stream_titles_are_reported() {
    let url = start_icy_server();
    let (player, events) = player();

    player.load(7, &url, 0).unwrap();
    let seen = wait_for_title(&events);

    assert!(seen.contains(&PlayerEvent::Started { uid: 7 }), "{seen:?}");
    assert_eq!(
        seen.last(),
        Some(&PlayerEvent::StreamTitle(Some(STREAM_TITLE.into()))),
        "{seen:?}"
    );
    player.stop().unwrap();
}
