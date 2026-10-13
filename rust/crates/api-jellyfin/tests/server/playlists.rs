use feishin_api::{ApiError, MusicServer};
use serde_json::json;
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{USER, authed, items_path, list, page, playlist_json, server, song_json};

async fn mount_playlist_page_and_items(mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path(items_path()))
        .and(query_param("IncludeItemTypes", "Playlist"))
        .and(query_param("MediaTypes", "Audio, Unknown"))
        .and(query_param("StartIndex", "0"))
        .and(query_param("Limit", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(vec![playlist_json("p1")], 3)))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/playlists/p1/items"))
        .and(query_param("UserId", USER))
        .and(query_param("IncludeItemTypes", "Audio"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(list(vec![song_json("s1", "a"), song_json("s2", "a")], 2)),
        )
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn playlist_list_and_songs() {
    let mock = MockServer::start().await;
    mount_playlist_page_and_items(&mock).await;

    let jellyfin = server(&mock);
    let playlists = jellyfin.playlist_list(page(0, 25)).await.unwrap();
    assert_eq!(playlists.total_count, Some(3));
    let playlist = &playlists.items[0];
    assert_eq!(playlist.name, "Playlist p1");
    assert_eq!(playlist.comment.as_deref(), Some("Mix"));
    assert_eq!(playlist.song_count, 2);
    assert_eq!(playlist.duration_ms, 491_000);
    assert_eq!(playlist.cover_art_id, None);

    let songs = jellyfin.playlist_songs("p1").await.unwrap();
    assert_eq!(songs.len(), 2);
}

async fn mount_playlist_creation(mock: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/playlists"))
        .and(authed())
        .and(body_json(
            json!({ "MediaType": "Audio", "Name": "New", "UserId": USER }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "Id": "p9" })))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/playlists/p9/items"))
        .and(query_param("Ids", "s1,s2"))
        .and(query_param("UserId", USER))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/users/{USER}/items/p9")))
        .and(query_param("Ids", "p9"))
        .respond_with(ResponseTemplate::new(200).set_body_json(playlist_json("p9")))
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn create_playlist_adds_songs_and_returns_detail() {
    let mock = MockServer::start().await;
    mount_playlist_creation(&mock).await;

    let playlist = server(&mock)
        .create_playlist("New", &["s1".into(), "s2".into()])
        .await
        .unwrap();
    assert_eq!(playlist.id, "p9");
}

#[tokio::test]
async fn add_to_playlist_batches_ids() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/playlists/p1/items"))
        .and(authed())
        .respond_with(ResponseTemplate::new(204))
        .expect(3)
        .mount(&mock)
        .await;

    let song_ids: Vec<String> = (0..120).map(|index| format!("s{index}")).collect();
    server(&mock)
        .add_to_playlist("p1", &song_ids)
        .await
        .unwrap();
}

#[tokio::test]
async fn remove_from_playlist_maps_indexes_to_entry_ids() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/playlists/p1/items"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(
            vec![
                song_json("s1", "a"),
                song_json("s2", "a"),
                song_json("s3", "a"),
            ],
            3,
        )))
        .mount(&mock)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/playlists/p1/items"))
        .and(query_param("EntryIds", "entry-s1,entry-s3"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock)
        .await;

    let jellyfin = server(&mock);
    jellyfin.remove_from_playlist("p1", &[0, 2]).await.unwrap();
    assert!(matches!(
        jellyfin.remove_from_playlist("p1", &[9]).await,
        Err(ApiError::NotFound)
    ));
}

#[tokio::test]
async fn delete_playlist() {
    let mock = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/items/p1"))
        .and(authed())
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock)
        .await;

    server(&mock).delete_playlist("p1").await.unwrap();
}
