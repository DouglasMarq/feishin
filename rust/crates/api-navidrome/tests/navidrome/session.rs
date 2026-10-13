use feishin_api::{ApiError, Credential, MusicServer};
use feishin_api_navidrome::NavidromeServer;
use serde_json::{Value, json};
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{SUBSONIC_AUTH, config, nd_song, server, subsonic_failed, subsonic_ok};

async fn mount_login(mock: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .and(body_json(
            json!({ "username": "alice", "password": "secret" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "u1",
            "isAdmin": false,
            "name": "Alice",
            "subsonicSalt": "salt1",
            "subsonicToken": "tok1",
            "token": "jwt-1",
            "username": "alice"
        })))
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn login_builds_navidrome_credential() {
    let mock = MockServer::start().await;
    mount_login(&mock).await;

    let (server, credential) = NavidromeServer::login(config(&mock.uri()), "secret")
        .await
        .expect("login");

    assert_eq!(
        credential,
        Credential::Navidrome {
            subsonic: SUBSONIC_AUTH.into(),
            nd_token: Some("jwt-1".into()),
        }
    );
    assert_eq!(server.credential(), credential);
    assert_eq!(server.config().id, "srv1");
}

#[tokio::test]
async fn login_rejected_is_auth_error() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({ "error": "Invalid" })))
        .mount(&mock)
        .await;

    let result = NavidromeServer::login(config(&mock.uri()), "wrong").await;
    assert!(matches!(result, Err(ApiError::Auth(_))));
}

#[tokio::test]
async fn with_credential_rejects_other_backends() {
    let result = NavidromeServer::with_credential(
        config("http://localhost:1"),
        &Credential::Subsonic {
            subsonic: SUBSONIC_AUTH.into(),
        },
    );
    assert!(matches!(result, Err(ApiError::Auth(_))));
}

#[tokio::test]
async fn native_calls_pick_up_refreshed_token() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/song/s1"))
        .and(header("x-nd-authorization", "Bearer jwt-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-nd-authorization", "jwt-2")
                .set_body_json(nd_song("s1", "First")),
        )
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/song/s2"))
        .and(header("x-nd-authorization", "Bearer jwt-2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(nd_song("s2", "Second")))
        .expect(1)
        .mount(&mock)
        .await;

    let server = server(&mock, Some("jwt-1"));
    assert_eq!(server.song("s1").await.expect("s1").title, "First");
    assert_eq!(
        server.credential(),
        Credential::Navidrome {
            subsonic: SUBSONIC_AUTH.into(),
            nd_token: Some("jwt-2".into()),
        }
    );
    assert_eq!(server.song("s2").await.expect("s2").title, "Second");
}

fn subsonic_song_fixture() -> Value {
    json!({
        "id": "s1",
        "parent": "al1",
        "isDir": false,
        "title": "From Subsonic",
        "album": "Album One",
        "artist": "Artist One",
        "artistId": "ar1",
        "albumId": "al1",
        "coverArt": "mf-s1",
        "size": 100,
        "contentType": "audio/mpeg",
        "suffix": "mp3",
        "duration": 200,
        "path": "a/b.mp3",
        "created": "2024-01-01T00:00:00Z",
        "type": "music",
        "isVideo": false,
        "discNumber": 1,
        "track": 3,
        "artists": [{ "id": "ar1", "name": "Artist One" }],
        "albumArtists": [{ "id": "ar1", "name": "Artist One" }]
    })
}

async fn mount_expired_native_and_subsonic_song(mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/song/s1"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/getSong.view"))
        .and(query_param("id", "s1"))
        .and(query_param("u", "alice"))
        .and(query_param("s", "salt1"))
        .and(query_param("t", "tok1"))
        .and(query_param("v", "1.16.1"))
        .and(query_param("c", "feishin"))
        .and(query_param("f", "json"))
        .respond_with(subsonic_ok(json!({ "song": subsonic_song_fixture() })))
        .expect(2)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn rejected_token_falls_back_to_subsonic() {
    let mock = MockServer::start().await;
    mount_expired_native_and_subsonic_song(&mock).await;

    let server = server(&mock, Some("expired"));
    let song = server.song("s1").await.expect("fallback song");
    assert_eq!(song.title, "From Subsonic");
    assert_eq!(song.cover_art_id.as_deref(), Some("mf-s1"));
    assert_eq!(song.track_number, Some(3));
    assert_eq!(
        server.credential(),
        Credential::Navidrome {
            subsonic: SUBSONIC_AUTH.into(),
            nd_token: None,
        }
    );
    server
        .song("s1")
        .await
        .expect("second call goes straight to subsonic");
}

#[tokio::test]
async fn subsonic_errors_are_mapped() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/scrobble.view"))
        .respond_with(subsonic_failed(70, "not found"))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/ping.view"))
        .respond_with(subsonic_failed(40, "Wrong username or password"))
        .mount(&mock)
        .await;

    let server = server(&mock, None);
    assert!(matches!(
        server.scrobble("x", true, None).await,
        Err(ApiError::NotFound)
    ));
    assert!(matches!(server.ping().await, Err(ApiError::Auth(_))));
}
