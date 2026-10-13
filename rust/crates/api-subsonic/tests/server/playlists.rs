use feishin_api::MusicServer;
use serde_json::json;
use wiremock::matchers::{path, query_param};
use wiremock::{Mock, MockServer};

use crate::support::{all_query_values, ok, paging, playlist, server, song};

async fn mount_playlist_reads(mock: &MockServer) {
    Mock::given(path("/rest/getPlaylists.view"))
        .respond_with(ok(json!({
            "playlists": { "playlist": [playlist("p1", "One"), playlist("p2", "Two"), playlist("p3", "Three")] }
        })))
        .mount(mock)
        .await;
    let mut detail = playlist("p1", "One");
    detail["entry"] = json!([song("s1", "A"), song("s2", "B")]);
    Mock::given(path("/rest/getPlaylist.view"))
        .and(query_param("id", "p1"))
        .respond_with(ok(json!({ "playlist": detail })))
        .mount(mock)
        .await;
}

async fn mount_playlist_writes(mock: &MockServer) {
    Mock::given(path("/rest/createPlaylist.view"))
        .and(query_param("name", "New list"))
        .and(all_query_values("songId", vec!["s1", "s2"]))
        .respond_with(ok(json!({ "playlist": playlist("p9", "New list") })))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(path("/rest/updatePlaylist.view"))
        .and(query_param("playlistId", "p1"))
        .and(all_query_values("songIdToAdd", vec!["s3", "s4"]))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(path("/rest/updatePlaylist.view"))
        .and(query_param("playlistId", "p1"))
        .and(all_query_values("songIndexToRemove", vec!["0", "2"]))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(path("/rest/deletePlaylist.view"))
        .and(query_param("id", "p1"))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn playlists_crud() {
    let mock = MockServer::start().await;
    mount_playlist_reads(&mock).await;
    mount_playlist_writes(&mock).await;
    let server = server(&mock);

    let page = server.playlist_list(paging(1, 1)).await.expect("playlists");
    assert_eq!(page.total_count, Some(3));
    let second = &page.items[0];
    assert_eq!(second.id, "p2");
    assert_eq!(second.owner.as_deref(), Some("alice"));
    assert!(second.public);
    assert_eq!(second.duration_ms, 360_000);
    assert_eq!(second.updated_at.as_deref(), Some("2024-02-01T00:00:00Z"));
    assert_eq!(second.cover_art_id.as_deref(), Some("pl-p2"));

    let songs = server.playlist_songs("p1").await.expect("songs");
    assert_eq!(songs.len(), 2);

    let created = server
        .create_playlist("New list", &["s1".into(), "s2".into()])
        .await
        .expect("create");
    assert_eq!(created.id, "p9");

    assert!(
        server
            .add_to_playlist("p1", &["s3".into(), "s4".into()])
            .await
            .is_ok()
    );
    assert!(server.remove_from_playlist("p1", &[0, 2]).await.is_ok());
    assert!(server.delete_playlist("p1").await.is_ok());
}
