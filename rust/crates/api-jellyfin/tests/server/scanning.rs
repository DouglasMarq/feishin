use feishin_api::MusicServer;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{authed, server};

#[tokio::test]
async fn starting_a_scan_refreshes_the_library() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/library/refresh"))
        .and(authed())
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock)
        .await;

    let scan = server(&mock).start_library_scan().await.unwrap();

    assert!(scan.scanning);
}

#[tokio::test]
async fn scan_status_follows_the_refresh_library_task() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/scheduledtasks"))
        .and(query_param("IsHidden", "false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "Key": "CleanCache", "State": "Idle" },
            { "Key": "RefreshLibrary", "State": "Running" }
        ])))
        .expect(1)
        .mount(&mock)
        .await;

    let scan = server(&mock).library_scan().await.unwrap();

    assert!(scan.scanning);
}
