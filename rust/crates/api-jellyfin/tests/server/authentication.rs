use feishin_api::{ApiError, Credential, MusicServer};
use feishin_api_jellyfin::{JellyfinServer, auth_header};
use serde_json::json;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{AuthHeader, TOKEN, USER, authed, config, server};

async fn mount_successful_login(mock: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/users/authenticatebyname"))
        .and(AuthHeader(auth_header("srv1", None)))
        .and(body_json(json!({ "Pw": "secret", "Username": "alice" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "AccessToken": TOKEN,
            "ServerId": "s",
            "SessionInfo": {},
            "User": { "Id": USER, "Name": "alice", "Policy": { "IsAdministrator": false } }
        })))
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn login_sends_mediabrowser_header_and_returns_credential() {
    let mock = MockServer::start().await;
    mount_successful_login(&mock).await;
    Mock::given(method("GET"))
        .and(path("/system/info"))
        .and(authed())
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "Version": "10.10.0" })))
        .expect(1)
        .mount(&mock)
        .await;

    let (jellyfin, credential) = JellyfinServer::login(config(&mock.uri()), "secret")
        .await
        .unwrap();
    assert_eq!(
        credential,
        Credential::Jellyfin {
            access_token: TOKEN.into(),
            user_id: USER.into()
        }
    );
    assert_eq!(jellyfin.user_id(), USER);
    jellyfin.ping().await.unwrap();
}

#[test]
fn auth_header_matches_feishin_format() {
    let header = auth_header("dev", Some("t"));
    assert!(header.starts_with(
        r#"MediaBrowser Client="Feishin", Device="Desktop Client", DeviceId="dev", Version=""#
    ));
    assert!(header.ends_with(r#"", Token="t""#));
    assert!(!auth_header("dev", None).contains("Token"));
}

#[tokio::test]
async fn login_failure_is_auth_error() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/users/authenticatebyname"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Invalid username or password"))
        .mount(&mock)
        .await;

    let result = JellyfinServer::login(config(&mock.uri()), "bad").await;
    assert!(matches!(result, Err(ApiError::Auth(_))));
}

#[test]
fn with_credential_rejects_other_backends() {
    let credential = Credential::Subsonic {
        subsonic: "u=a&p=b".into(),
    };
    assert!(JellyfinServer::with_credential(config("http://x"), &credential).is_err());
}

#[tokio::test]
async fn unauthorized_maps_to_auth_error() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/users/{USER}/items/s1")))
        .respond_with(ResponseTemplate::new(401))
        .mount(&mock)
        .await;

    assert!(matches!(
        server(&mock).song("s1").await,
        Err(ApiError::Auth(_))
    ));
}
