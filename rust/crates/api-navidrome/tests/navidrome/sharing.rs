use feishin_api::{ApiError, MusicServer};
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{server, subsonic_ok};

#[tokio::test]
async fn a_share_link_comes_from_create_share() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/createShare.view"))
        .and(query_param("id", "al1"))
        .respond_with(subsonic_ok(json!({
            "shares": { "share": [{
                "id": "Xy12", "url": "http://localhost/share/Xy12", "description": "",
                "username": "alice", "created": "2024-01-01T00:00:00Z", "visitCount": 0
            }] }
        })))
        .expect(1)
        .mount(&mock)
        .await;
    let navidrome = server(&mock, Some("jwt-1"));

    let share = navidrome
        .create_share(&["al1".to_owned()], None)
        .await
        .unwrap();

    assert!(navidrome.can_share());
    assert_eq!(share.url, "http://localhost/share/Xy12");
    assert_eq!(share.description, None);
}

#[tokio::test]
async fn disabled_sharing_is_reported_as_unsupported() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/createShare.view"))
        .respond_with(
            ResponseTemplate::new(501)
                .set_body_string("This endpoint is not implemented, but may be in future releases"),
        )
        .mount(&mock)
        .await;

    let refused = server(&mock, Some("jwt-1"))
        .create_share(&["s1".to_owned()], None)
        .await;

    assert!(matches!(refused, Err(ApiError::Unsupported(_))));
}
