use feishin_api::MusicServer;
use serde_json::json;
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{AudioFormat, mount_ping, nd_song, server, subsonic_entry, subsonic_ok};

async fn mount_index_queue_save_and_native_queue(mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/savePlayQueueByIndex.view"))
        .and(query_param("id", "s1"))
        .and(query_param("id", "s2"))
        .and(query_param("currentIndex", "1"))
        .and(query_param("position", "12345"))
        .respond_with(subsonic_ok(json!({})))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/queue"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "q1",
            "userId": "u1",
            "current": 1,
            "position": 12345,
            "changedBy": "feishin",
            "createdAt": "2024-01-01T00:00:00Z",
            "updatedAt": "2024-01-01T00:00:00Z",
            "items": [nd_song("s1", "One"), nd_song("s2", "Two")]
        })))
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn play_queue_prefers_opensubsonic_index_queue_for_saving() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.58.0", &["indexBasedQueue"]).await;
    mount_index_queue_save_and_native_queue(&mock).await;

    let server = server(&mock, Some("jwt-1"));
    server
        .save_play_queue(&["s1".into(), "s2".into()], Some("s2"), 12_345)
        .await
        .expect("save");
    let queue = server.play_queue().await.expect("queue").expect("some");
    assert_eq!(queue.songs.len(), 2);
    assert_eq!(queue.current_song_id.as_deref(), Some("s2"));
    assert_eq!(queue.position_ms, 12_345);
}

#[tokio::test]
async fn play_queue_uses_native_api_without_index_extension() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.57.0", &[]).await;
    Mock::given(method("POST"))
        .and(path("/api/queue"))
        .and(body_json(
            json!({ "ids": ["s1"], "current": 0, "position": 5 }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(null)))
        .expect(1)
        .mount(&mock)
        .await;

    server(&mock, Some("jwt-1"))
        .save_play_queue(&["s1".into()], Some("s1"), 5)
        .await
        .expect("save");
}

async fn mount_legacy_queue(mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/getPlayQueue.view"))
        .respond_with(subsonic_ok(json!({
            "playQueue": {
                "current": "s7",
                "position": 99,
                "username": "alice",
                "changed": "2024-01-01T00:00:00Z",
                "changedBy": "feishin",
                "entry": [subsonic_entry("s7", "Seven", "p", AudioFormat::Mp3)]
            }
        })))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/savePlayQueue.view"))
        .and(query_param("current", "s7"))
        .and(query_param("id", "s7"))
        .respond_with(subsonic_ok(json!({})))
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn play_queue_legacy_subsonic() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.50.0", &[]).await;
    mount_legacy_queue(&mock).await;

    let server = server(&mock, Some("jwt-1"));
    let queue = server.play_queue().await.expect("queue").expect("some");
    assert_eq!(queue.current_song_id.as_deref(), Some("s7"));
    assert_eq!(queue.position_ms, 99);
    server
        .save_play_queue(&["s7".into()], Some("s7"), 99)
        .await
        .expect("save");
}
