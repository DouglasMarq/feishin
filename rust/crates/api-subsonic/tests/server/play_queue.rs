use feishin_api::MusicServer;
use serde_json::json;
use wiremock::matchers::{body_string_contains, method, path, query_param};
use wiremock::{Mock, MockServer};

use crate::support::{all_query_values, mount_extensions, mount_plain_ping, ok, server, song};

async fn mount_id_based_play_queue(mock: &MockServer) {
    Mock::given(path("/rest/getOpenSubsonicExtensions.view"))
        .respond_with(ok(json!({})))
        .expect(0)
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/savePlayQueue.view"))
        .and(all_query_values("id", vec!["s1", "s2", "s3"]))
        .and(query_param("current", "s2"))
        .and(query_param("position", "42000"))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(path("/rest/getPlayQueue.view"))
        .respond_with(ok(json!({
            "playQueue": {
                "current": "s2", "position": 42000, "username": "alice",
                "changed": "2024-01-01T00:00:00Z", "changedBy": "Feishin",
                "entry": [song("s1", "A"), song("s2", "B")]
            }
        })))
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn play_queue_on_plain_subsonic() {
    let mock = MockServer::start().await;
    mount_plain_ping(&mock).await;
    mount_id_based_play_queue(&mock).await;
    let server = server(&mock);

    let ids: Vec<String> = vec!["s1".into(), "s2".into(), "s3".into()];
    assert!(
        server
            .save_play_queue(&ids, Some("s2"), 42_000)
            .await
            .is_ok()
    );
    let queue = server
        .play_queue()
        .await
        .expect("queue")
        .expect("some queue");
    assert_eq!(queue.songs.len(), 2);
    assert_eq!(queue.current_song_id.as_deref(), Some("s2"));
    assert_eq!(queue.position_ms, 42_000);
    assert_eq!(server.extensions().await.ok(), Some(&Default::default()));
}

async fn mount_index_based_play_queue(mock: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/rest/savePlayQueueByIndex.view"))
        .and(body_string_contains("u=alice&s=abc123"))
        .and(body_string_contains(
            "&id=s1&id=s2&position=1500&currentIndex=1",
        ))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/getPlayQueueByIndex.view"))
        .respond_with(ok(json!({
            "playQueueByIndex": {
                "currentIndex": 1, "position": 1500, "username": "alice",
                "changed": "2024-01-01T00:00:00Z", "changedBy": "Feishin",
                "entry": [song("s1", "A"), song("s2", "B")]
            }
        })))
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn play_queue_by_index_with_form_post() {
    let mock = MockServer::start().await;
    mount_extensions(
        &mock,
        json!([
            { "name": "indexBasedQueue", "versions": [1] },
            { "name": "formPost", "versions": [1] }
        ]),
    )
    .await;
    mount_index_based_play_queue(&mock).await;
    let server = server(&mock);

    let ids: Vec<String> = vec!["s1".into(), "s2".into()];
    assert!(
        server
            .save_play_queue(&ids, Some("s2"), 1_500)
            .await
            .is_ok()
    );
    let queue = server
        .play_queue()
        .await
        .expect("queue")
        .expect("some queue");
    assert_eq!(queue.current_song_id.as_deref(), Some("s2"));
    assert_eq!(queue.position_ms, 1_500);
}

fn many_song_ids() -> Vec<String> {
    (0..2_000).map(|index| format!("song-{index}")).collect()
}

#[tokio::test]
async fn huge_play_queue_is_skipped_without_form_post() {
    let mock = MockServer::start().await;
    mount_plain_ping(&mock).await;
    Mock::given(path("/rest/savePlayQueue.view"))
        .respond_with(ok(json!({})))
        .expect(0)
        .mount(&mock)
        .await;
    let saved = server(&mock)
        .save_play_queue(&many_song_ids(), Some("song-3"), 10)
        .await;
    assert!(saved.is_ok());
}

#[tokio::test]
async fn huge_play_queue_is_posted_with_form_post() {
    let mock = MockServer::start().await;
    mount_extensions(&mock, json!([{ "name": "formPost", "versions": [1] }])).await;
    Mock::given(method("POST"))
        .and(path("/rest/savePlayQueue.view"))
        .and(body_string_contains(
            "&id=song-1999&position=10&current=song-3",
        ))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    let saved = server(&mock)
        .save_play_queue(&many_song_ids(), Some("song-3"), 10)
        .await;
    assert!(saved.is_ok());
}

#[tokio::test]
async fn empty_play_queue_is_none() {
    let mock = MockServer::start().await;
    mount_extensions(
        &mock,
        json!([{ "name": "indexBasedQueue", "versions": [1] }]),
    )
    .await;
    Mock::given(path("/rest/getPlayQueueByIndex.view"))
        .respond_with(ok(json!({})))
        .mount(&mock)
        .await;
    assert_eq!(server(&mock).play_queue().await.ok(), Some(None));
}
