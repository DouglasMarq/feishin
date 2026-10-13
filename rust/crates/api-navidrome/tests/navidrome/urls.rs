use feishin_api::{MusicServer, StreamOptions};
use feishin_api_navidrome::NavidromeServer;
use feishin_core::model::Song;

use crate::support::{config, credential};

fn song_with_spaced_id() -> Song {
    Song {
        id: "s 1".into(),
        server_id: "srv1".into(),
        title: "t".into(),
        artists: Vec::new(),
        album_artists: Vec::new(),
        album: None,
        album_id: None,
        duration_ms: 0,
        track_number: None,
        disc_number: None,
        year: None,
        genres: Vec::new(),
        bitrate_kbps: None,
        play_count: 0,
        user_rating: None,
        favorite: false,
        cover_art_id: None,
        replay_gain: None,
        path: None,
        size_bytes: None,
        container: None,
        created_at: None,
        last_played_at: None,
    }
}

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
    let song = song_with_spaced_id();

    assert_eq!(
        server.stream_url(&song, &StreamOptions::default()),
        "https://music.example.com/rest/stream.view?u=alice&s=salt1&t=tok1&v=1.16.1&c=feishin&id=s+1"
    );
    assert_eq!(
        server.stream_url(
            &song,
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
