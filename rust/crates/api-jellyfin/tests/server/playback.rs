use feishin_api::{ApiError, MusicServer};
use feishin_core::model::ItemKind;
use serde_json::json;
use wiremock::matchers::{body_json, body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{USER, authed, server, song_json};

async fn mount_favorite_endpoints(mock: &MockServer) {
    Mock::given(method("POST"))
        .and(path(format!("/users/{USER}/favoriteitems/s1")))
        .and(authed())
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "IsFavorite": true })))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("/users/{USER}/favoriteitems/s2")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "IsFavorite": true })))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("DELETE"))
        .and(path(format!("/users/{USER}/favoriteitems/al1")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "IsFavorite": false })))
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn favorites_and_rating() {
    let mock = MockServer::start().await;
    mount_favorite_endpoints(&mock).await;

    let jellyfin = server(&mock);
    jellyfin
        .set_favorite(ItemKind::Song, &["s1".into(), "s2".into()], true)
        .await
        .unwrap();
    jellyfin
        .set_favorite(ItemKind::Album, &["al1".into()], false)
        .await
        .unwrap();
    assert!(matches!(
        jellyfin.set_rating(ItemKind::Song, "s1", 4).await,
        Err(ApiError::Unsupported(_))
    ));
}

async fn mount_scrobble_endpoints(mock: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/sessions/playing"))
        .and(authed())
        .and(body_json(json!({ "ItemId": "s1", "PositionTicks": 0 })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/users/{USER}/items/s1")))
        .respond_with(ResponseTemplate::new(200).set_body_json(song_json("s1", "a")))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/sessions/playing/stopped"))
        .and(body_json(json!({
            "ItemId": "s1",
            "IsPaused": true,
            "PositionTicks": 2_455_000_000_u64
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(mock)
        .await;
}

async fn mount_progress_endpoint(mock: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/sessions/playing/progress"))
        .and(body_partial_json(json!({
            "ItemId": "s1",
            "EventName": "pause",
            "IsPaused": true,
            "PositionTicks": 12_340_000
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn scrobble_reports_sessions() {
    let mock = MockServer::start().await;
    mount_scrobble_endpoints(&mock).await;
    mount_progress_endpoint(&mock).await;

    let jellyfin = server(&mock);
    jellyfin.scrobble("s1", false, None).await.unwrap();
    jellyfin
        .scrobble("s1", true, Some(1_700_000_000_000))
        .await
        .unwrap();
    jellyfin.report_progress("s1", 1234, true).await.unwrap();
}

#[tokio::test]
async fn play_queue_is_unsupported() {
    let mock = MockServer::start().await;
    let jellyfin = server(&mock);
    assert!(matches!(
        jellyfin.play_queue().await,
        Err(ApiError::Unsupported(_))
    ));
    assert!(matches!(
        jellyfin.save_play_queue(&[], None, 0).await,
        Err(ApiError::Unsupported(_))
    ));
}
