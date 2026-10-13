use feishin_api::{ApiError, MusicServer};
use feishin_core::model::ItemKind;
use serde_json::json;
use wiremock::matchers::{path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer};

use crate::support::{all_query_values, ok, server};

async fn mount_star_and_unstar(mock: &MockServer) {
    Mock::given(path("/rest/star.view"))
        .and(all_query_values("albumId", vec!["a1", "a2"]))
        .and(query_param_is_missing("id"))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(path("/rest/unstar.view"))
        .and(all_query_values("id", vec!["s1"]))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(path("/rest/star.view"))
        .and(all_query_values("artistId", vec!["ar-1"]))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(mock)
        .await;
}

async fn mount_ratings(mock: &MockServer) {
    Mock::given(path("/rest/setRating.view"))
        .and(query_param("id", "s1"))
        .and(query_param("rating", "5"))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(path("/rest/setRating.view"))
        .and(query_param("id", "a1"))
        .and(query_param("rating", "0"))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn favorites_and_rating() {
    let mock = MockServer::start().await;
    mount_star_and_unstar(&mock).await;
    mount_ratings(&mock).await;
    let server = server(&mock);

    assert!(
        server
            .set_favorite(ItemKind::Album, &["a1".into(), "a2".into()], true)
            .await
            .is_ok()
    );
    assert!(
        server
            .set_favorite(ItemKind::Song, &["s1".into()], false)
            .await
            .is_ok()
    );
    assert!(
        server
            .set_favorite(ItemKind::Artist, &["ar-1".into()], true)
            .await
            .is_ok()
    );
    assert!(server.set_favorite(ItemKind::Song, &[], true).await.is_ok());
    assert!(matches!(
        server
            .set_favorite(ItemKind::Playlist, &["p1".into()], true)
            .await,
        Err(ApiError::Unsupported(_))
    ));

    assert!(server.set_rating(ItemKind::Song, "s1", 9).await.is_ok());
    assert!(server.set_rating(ItemKind::Album, "a1", 0).await.is_ok());
    assert!(matches!(
        server.set_rating(ItemKind::Playlist, "p1", 3).await,
        Err(ApiError::Unsupported(_))
    ));
}

#[tokio::test]
async fn scrobble_now_playing_and_submission() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/scrobble.view"))
        .and(query_param("id", "s1"))
        .and(query_param("submission", "false"))
        .and(query_param_is_missing("time"))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(path("/rest/scrobble.view"))
        .and(query_param("id", "s1"))
        .and(query_param("submission", "true"))
        .and(query_param("time", "1700000000000"))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    let server = server(&mock);

    assert!(
        server
            .scrobble("s1", false, Some(1_700_000_000_000))
            .await
            .is_ok()
    );
    assert!(
        server
            .scrobble("s1", true, Some(1_700_000_000_000))
            .await
            .is_ok()
    );
}
