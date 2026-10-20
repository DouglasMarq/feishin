use feishin_api::{ApiError, MusicServer};
use serde_json::json;
use wiremock::matchers::{path, query_param};
use wiremock::{Mock, MockServer};

use crate::support::{all_query_values, failed, ok, server};

#[tokio::test]
async fn creating_a_share_returns_its_public_link() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/createShare.view"))
        .and(all_query_values("id", vec!["s1", "s2"]))
        .and(query_param("description", "Road trip"))
        .respond_with(ok(json!({
            "shares": { "share": [{
                "id": 12, "url": "https://music.example/share/abc", "description": "Road trip",
                "username": "alice", "created": "2024-01-01T00:00:00Z",
                "expires": "2024-02-01T00:00:00Z", "visitCount": 0
            }] }
        })))
        .expect(1)
        .mount(&mock)
        .await;
    let server = server(&mock);

    let ids = ["s1".to_owned(), "s2".to_owned()];
    let share = server.create_share(&ids, Some("Road trip")).await.unwrap();

    assert!(server.can_share());
    assert_eq!(share.id, "12");
    assert_eq!(share.url, "https://music.example/share/abc");
    assert_eq!(share.expires_at.as_deref(), Some("2024-02-01T00:00:00Z"));
}

#[tokio::test]
async fn users_without_the_share_role_get_the_server_error() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/createShare.view"))
        .respond_with(failed(json!(50), "User is not authorized to share"))
        .mount(&mock)
        .await;

    let refused = server(&mock).create_share(&["al-1".to_owned()], None).await;

    assert!(matches!(refused, Err(ApiError::NotAuthorized(_))));
}

#[tokio::test]
async fn an_empty_share_list_is_an_error() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/createShare.view"))
        .respond_with(ok(json!({ "shares": {} })))
        .mount(&mock)
        .await;

    let created = server(&mock).create_share(&["al-1".to_owned()], None).await;

    assert!(matches!(created, Err(ApiError::Decode(_))));
}
