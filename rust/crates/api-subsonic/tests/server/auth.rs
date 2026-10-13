use feishin_api::{ApiError, Credential, MusicServer};
use feishin_api_subsonic::SubsonicServer;
use serde_json::json;
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{config, failed, ok, server, valid_salted_token};

#[tokio::test]
async fn login_uses_token_auth() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/getUser.view"))
        .and(query_param("u", "alice"))
        .and(query_param("username", "alice"))
        .and(query_param("v", "1.13.0"))
        .and(query_param("c", "Feishin"))
        .and(query_param("f", "json"))
        .and(query_param_is_missing("p"))
        .and(valid_salted_token("s3cret"))
        .respond_with(ok(
            json!({ "user": { "username": "alice", "adminRole": false } }),
        ))
        .expect(1)
        .mount(&mock)
        .await;

    let (server, credential) = SubsonicServer::login(config(&mock.uri()), "s3cret")
        .await
        .expect("login");
    let Credential::Subsonic { subsonic } = &credential else {
        panic!("unexpected credential {credential:?}");
    };
    assert!(subsonic.starts_with("u=alice&s="));
    assert!(subsonic.contains("&t="));
    assert_eq!(server.config().username, "alice");

    let restored = SubsonicServer::with_credential(config(&mock.uri()), &credential);
    assert!(restored.is_ok());
}

#[tokio::test]
async fn login_falls_back_to_plain_password() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/getUser.view"))
        .and(valid_salted_token("pa ss&word"))
        .respond_with(failed(json!(41), "Token authentication not supported"))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(path("/rest/getUser.view"))
        .and(query_param("p", "pa ss&word"))
        .and(query_param_is_missing("t"))
        .respond_with(ok(json!({ "user": { "username": "alice" } })))
        .expect(1)
        .mount(&mock)
        .await;

    let (_, credential) = SubsonicServer::login(config(&mock.uri()), "pa ss&word")
        .await
        .expect("login");
    assert_eq!(
        credential,
        Credential::Subsonic {
            subsonic: "u=alice&p=pa%20ss%26word".into()
        }
    );
}

#[tokio::test]
async fn login_with_wrong_password_is_auth_error() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/getUser.view"))
        .respond_with(failed(json!(40), "Wrong username or password"))
        .expect(2)
        .mount(&mock)
        .await;

    let result = SubsonicServer::login(config(&mock.uri()), "nope").await;
    assert!(
        matches!(result, Err(ApiError::Auth(message)) if message == "Wrong username or password")
    );
}

#[test]
fn with_credential_validates_input() {
    let server_config = config("http://localhost");
    let bad = Credential::Subsonic {
        subsonic: "u=alice".into(),
    };
    assert!(matches!(
        SubsonicServer::with_credential(server_config.clone(), &bad),
        Err(ApiError::Auth(_))
    ));
    let jellyfin = Credential::Jellyfin {
        access_token: "x".into(),
        user_id: "y".into(),
    };
    assert!(matches!(
        SubsonicServer::with_credential(server_config.clone(), &jellyfin),
        Err(ApiError::Auth(_))
    ));
    let legacy = Credential::Subsonic {
        subsonic: "u=alice&p=pw".into(),
    };
    assert!(SubsonicServer::with_credential(server_config, &legacy).is_ok());
}

async fn mount_get_song_failure(mock: &MockServer, id: &str, response: ResponseTemplate) {
    Mock::given(path("/rest/getSong.view"))
        .and(query_param("id", id))
        .respond_with(response)
        .mount(mock)
        .await;
}

async fn mount_error_envelopes(mock: &MockServer) {
    let server_error = failed(json!(10), "Required parameter is missing");
    mount_get_song_failure(mock, "server", server_error).await;
    let auth_error = failed(json!("40"), "Wrong username or password");
    mount_get_song_failure(mock, "auth", auth_error).await;
    mount_get_song_failure(mock, "missing", failed(json!(70), "Song not found")).await;
    mount_get_song_failure(mock, "soft", failed(json!(0), "generic")).await;
    let bad_gateway = ResponseTemplate::new(502).set_body_string("Bad Gateway");
    mount_get_song_failure(mock, "http", bad_gateway).await;
    Mock::given(path("/rest/getSimilarSongs.view"))
        .respond_with(failed(json!(0), "Last.fm not linked"))
        .mount(mock)
        .await;
}

#[tokio::test]
async fn error_envelope_maps_to_api_errors() {
    let mock = MockServer::start().await;
    mount_error_envelopes(&mock).await;
    let server = server(&mock);

    assert!(matches!(
        server.song("server").await,
        Err(ApiError::Server { code: 10, message }) if message == "Required parameter is missing"
    ));
    assert!(matches!(server.song("auth").await, Err(ApiError::Auth(_))));
    assert!(matches!(
        server.song("missing").await,
        Err(ApiError::NotFound)
    ));
    assert!(matches!(
        server.song("soft").await,
        Err(ApiError::Server { code: 0, .. })
    ));
    assert!(matches!(
        server.song("http").await,
        Err(ApiError::Server { code: 502, .. })
    ));
    assert_eq!(server.similar_songs("x", 5).await.ok(), Some(Vec::new()));
}

#[tokio::test]
async fn requests_carry_credential_and_ping_works() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/ping.view"))
        .and(query_param("u", "alice"))
        .and(query_param("s", "abc123"))
        .and(query_param("t", "0123456789abcdef0123456789abcdef"))
        .and(query_param("v", "1.13.0"))
        .and(query_param("c", "Feishin"))
        .and(query_param("f", "json"))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    assert!(server(&mock).ping().await.is_ok());
}
