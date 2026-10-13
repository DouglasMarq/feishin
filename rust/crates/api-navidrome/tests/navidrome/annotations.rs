use feishin_api::{ApiError, MusicServer};
use feishin_core::model::ItemKind;
use serde_json::json;
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer};

use crate::support::{server, subsonic_ok};

async fn mount_star_unstar_and_rating(mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/star.view"))
        .and(query_param("albumId", "al1"))
        .and(query_param("albumId", "al2"))
        .and(query_param("u", "alice"))
        .respond_with(subsonic_ok(json!({})))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/unstar.view"))
        .and(query_param("id", "s1"))
        .respond_with(subsonic_ok(json!({})))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/setRating.view"))
        .and(query_param("id", "ar1"))
        .and(query_param("rating", "5"))
        .respond_with(subsonic_ok(json!({})))
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn favorites_and_rating_use_subsonic() {
    let mock = MockServer::start().await;
    mount_star_unstar_and_rating(&mock).await;

    let server = server(&mock, Some("jwt-1"));
    server
        .set_favorite(ItemKind::Album, &["al1".into(), "al2".into()], true)
        .await
        .expect("star");
    server
        .set_favorite(ItemKind::Song, &["s1".into()], false)
        .await
        .expect("unstar");
    server
        .set_rating(ItemKind::Artist, "ar1", 9)
        .await
        .expect("rating");
    assert!(matches!(
        server
            .set_favorite(ItemKind::Playlist, &["pl1".into()], true)
            .await,
        Err(ApiError::Unsupported(_))
    ));
}

#[tokio::test]
async fn scrobble_now_playing_and_submission() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/scrobble.view"))
        .and(query_param("id", "s1"))
        .and(query_param("submission", "false"))
        .and(query_param_is_missing("time"))
        .respond_with(subsonic_ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/scrobble.view"))
        .and(query_param("id", "s1"))
        .and(query_param("submission", "true"))
        .and(query_param("time", "1700000000000"))
        .respond_with(subsonic_ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;

    let server = server(&mock, Some("jwt-1"));
    server
        .scrobble("s1", false, None)
        .await
        .expect("now playing");
    server
        .scrobble("s1", true, Some(1_700_000_000_000))
        .await
        .expect("submission");
}
