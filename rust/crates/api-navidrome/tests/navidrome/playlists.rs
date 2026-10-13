use feishin_api::MusicServer;
use feishin_core::model::Paging;
use serde_json::json;
use wiremock::matchers::{body_json, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{mount_ping, native_playlist_json, native_song_json, server, subsonic_ok};

async fn mount_playlist_page(mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/playlist"))
        .and(query_param("_start", "0"))
        .and(query_param("_end", "25"))
        .and(query_param("_sort", "name"))
        .and(query_param("_order", "ASC"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-total-count", "3")
                .set_body_json(json!([native_playlist_json("pl1", "Mix")])),
        )
        .expect(1)
        .mount(mock)
        .await;
}

async fn mount_playlist_tracks(mock: &MockServer) {
    let mut track = native_song_json("1", "One");
    track["mediaFileId"] = json!("s1");
    track["playlistId"] = json!("pl1");
    Mock::given(method("GET"))
        .and(path("/api/playlist/pl1/tracks"))
        .and(query_param("_sort", "id"))
        .and(query_param("_end", "-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([track])))
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn playlist_list_and_songs() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.58.0", &[]).await;
    mount_playlist_page(&mock).await;
    mount_playlist_tracks(&mock).await;

    let server = server(&mock, Some("jwt-1"));
    let page = server
        .playlist_list(Paging {
            start_index: 0,
            limit: 25,
        })
        .await
        .expect("playlists");
    assert_eq!(page.total_count, Some(3));
    let playlist = &page.items[0];
    assert_eq!(playlist.name, "Mix");
    assert_eq!(playlist.owner.as_deref(), Some("alice"));
    assert_eq!(playlist.comment, None);
    assert_eq!(playlist.duration_ms, 600_000);
    assert_eq!(
        playlist.cover_art_id.as_deref(),
        Some("pl1&_=2024-02-02T00:00:00Z")
    );

    let songs = server.playlist_songs("pl1").await.expect("tracks");
    assert_eq!(songs[0].id, "s1");
}

async fn mount_create_and_add(mock: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/api/playlist"))
        .and(body_json(json!({ "name": "New", "rules": null })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "pl9" })))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/playlist/pl9/tracks"))
        .and(body_json(json!({ "ids": ["s1", "s2"] })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "added": 2 })))
        .expect(2)
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/playlist/pl9"))
        .respond_with(ResponseTemplate::new(200).set_body_json(native_playlist_json("pl9", "New")))
        .expect(1)
        .mount(mock)
        .await;
}

async fn mount_remove_and_delete(mock: &MockServer) {
    let mut first = native_song_json("7", "One");
    first["mediaFileId"] = json!("s1");
    let mut second = native_song_json("8", "Two");
    second["mediaFileId"] = json!("s2");
    Mock::given(method("GET"))
        .and(path("/api/playlist/pl9/tracks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([first, second])))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/playlist/pl9/tracks"))
        .and(query_param("id", "8"))
        .and(query_param_is_missing("_start"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ids": ["8"] })))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/playlist/pl9"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(null)))
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn playlist_create_add_remove_delete() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.58.0", &[]).await;
    mount_create_and_add(&mock).await;
    mount_remove_and_delete(&mock).await;

    let server = server(&mock, Some("jwt-1"));
    let ids = vec!["s1".to_owned(), "s2".to_owned()];
    let created = server.create_playlist("New", &ids).await.expect("create");
    assert_eq!(created.id, "pl9");
    assert_eq!(created.name, "New");
    server.add_to_playlist("pl9", &ids).await.expect("add");
    server
        .remove_from_playlist("pl9", &[1])
        .await
        .expect("remove");
    assert!(matches!(
        server.remove_from_playlist("pl9", &[]).await,
        Ok(())
    ));
    server.delete_playlist("pl9").await.expect("delete");
}

#[tokio::test]
async fn playlist_ops_without_session_use_subsonic() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.58.0", &[]).await;
    Mock::given(method("GET"))
        .and(path("/rest/updatePlaylist.view"))
        .and(query_param("playlistId", "pl1"))
        .and(query_param("songIndexToRemove", "2"))
        .respond_with(subsonic_ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/deletePlaylist.view"))
        .and(query_param("id", "pl1"))
        .respond_with(subsonic_ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;

    let server = server(&mock, None);
    server
        .remove_from_playlist("pl1", &[2])
        .await
        .expect("remove");
    server.delete_playlist("pl1").await.expect("delete");
}
