use feishin_api::{ApiError, MusicServer, StreamOptions, TranscodeFormat};
use feishin_api_navidrome::NavidromeServer;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{config, credential, server};

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
                format: TranscodeFormat::parse("opus"),
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

#[tokio::test]
async fn given_an_id_with_reserved_characters_the_path_is_encoded() {
    let mock = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/playlist/pl%2F1%3Fx%23y"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&mock)
        .await;

    server(&mock, Some("jwt-1"))
        .delete_playlist("pl/1?x#y")
        .await
        .expect("delete");
}

#[tokio::test]
async fn given_an_invalid_server_address_native_calls_fail() {
    for address in ["not a url", "mailto:feishin@navidrome.example"] {
        let server = NavidromeServer::with_credential(config(address), &credential(Some("jwt-1")))
            .expect("server");
        let fetched = server.song("s1").await;
        assert!(
            matches!(fetched, Err(ApiError::InvalidAddress(_))),
            "{address}"
        );
    }
}
