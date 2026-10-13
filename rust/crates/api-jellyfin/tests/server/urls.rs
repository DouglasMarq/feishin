use feishin_api::{MusicServer, StreamOptions};

use crate::support::server_at;

const SERVER_URL: &str = "https://jf.example";

#[test]
fn stream_url_downloads_original_without_options() {
    let jellyfin = server_at(SERVER_URL);
    assert_eq!(
        jellyfin.stream_url("s1", &StreamOptions::default()),
        "https://jf.example/Items/s1/Download?apiKey=tok123&playSessionId="
    );
}

#[test]
fn stream_url_transcodes_to_requested_format_and_bitrate() {
    let jellyfin = server_at(SERVER_URL);
    let options = StreamOptions {
        max_bitrate_kbps: Some(192),
        format: Some("opus".into()),
    };
    assert_eq!(
        jellyfin.stream_url("s1", &options),
        "https://jf.example/audio/s1/universal?userId=u1&deviceId=&audioCodec=opus&apiKey=tok123\
         &playSessionId=feishin-s1-opus-0-0&container=opus&transcodingProtocol=http\
         &transcodingContainer=opus&maxStreamingBitrate=192000"
    );
}

#[test]
fn stream_url_transcodes_to_mp3_when_only_bitrate_is_set() {
    let jellyfin = server_at(SERVER_URL);
    let options = StreamOptions {
        max_bitrate_kbps: Some(128),
        format: None,
    };
    assert_eq!(
        jellyfin.stream_url("s1", &options),
        "https://jf.example/audio/s1/universal?userId=u1&deviceId=&audioCodec=mp3&apiKey=tok123\
         &playSessionId=feishin-s1-mp3-0-0&container=mp3&transcodingProtocol=http\
         &transcodingContainer=mp3&maxStreamingBitrate=128000"
    );
}

#[test]
fn cover_art_url_adds_size_only_when_requested() {
    let jellyfin = server_at(SERVER_URL);
    assert_eq!(
        jellyfin.cover_art_url("al1", Some(300)),
        "https://jf.example/Items/al1/Images/Primary?quality=96&fillHeight=300&fillWidth=300"
    );
    assert_eq!(
        jellyfin.cover_art_url("al1", None),
        "https://jf.example/Items/al1/Images/Primary?quality=96"
    );
}
