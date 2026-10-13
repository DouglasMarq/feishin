use feishin_api::{MusicServer, StreamOptions};
use feishin_api_navidrome::NavidromeServer;

use crate::support::{config, credential};

fn remote_server() -> NavidromeServer {
    NavidromeServer::with_credential(
        config("https://music.example.com"),
        &credential(Some("jwt-1")),
    )
    .expect("server")
}

#[tokio::test]
async fn stream_url_includes_credentials() {
    let server = remote_server();

    assert_eq!(
        server.stream_url("s 1", &StreamOptions::default()),
        "https://music.example.com/rest/stream.view?u=alice&s=salt1&t=tok1&v=1.16.1&c=feishin&id=s+1"
    );
    assert_eq!(
        server.stream_url(
            "s 1",
            &StreamOptions {
                max_bitrate_kbps: Some(192),
                format: Some("opus".into()),
            }
        ),
        "https://music.example.com/rest/stream.view?u=alice&s=salt1&t=tok1&v=1.16.1&c=feishin&id=s+1&format=opus&maxBitRate=192"
    );
}

#[tokio::test]
async fn cover_art_url_includes_credentials() {
    let server = remote_server();

    assert_eq!(
        server.cover_art_url("al-1", Some(300)),
        "https://music.example.com/rest/getCoverArt.view?u=alice&s=salt1&t=tok1&v=1.16.1&c=feishin&id=al-1&size=300"
    );
    assert_eq!(
        server.cover_art_url("pl1&_=2024-02-02T00:00:00Z", None),
        "https://music.example.com/rest/getCoverArt.view?u=alice&s=salt1&t=tok1&v=1.16.1&c=feishin&id=pl1&_=2024-02-02T00%3A00%3A00Z"
    );
}
