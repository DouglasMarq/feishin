use feishin_api::MusicServer;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{USER, list, page, playlist_json, server, song_json};

async fn mount_playlist_item_page(mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/playlists/p1/items"))
        .and(query_param("UserId", USER))
        .and(query_param("IncludeItemTypes", "Audio"))
        .and(query_param("StartIndex", "100"))
        .and(query_param("Limit", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(
            vec![song_json("s101", "a"), song_json("s102", "a")],
            250,
        )))
        .expect(1)
        .mount(mock)
        .await;
}

async fn mount_playlist_summary(mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path(format!("/users/{USER}/items/p1")))
        .and(query_param("Ids", "p1"))
        .and(query_param("Fields", "Overview"))
        .respond_with(ResponseTemplate::new(200).set_body_json(playlist_json("p1")))
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn playlist_items_are_paged_with_start_index_and_limit() {
    let mock = MockServer::start().await;
    mount_playlist_item_page(&mock).await;

    let songs = server(&mock)
        .playlist_song_page("p1", page(100, 100))
        .await
        .unwrap();
    assert_eq!(songs.total_count, Some(250));
    assert_eq!(songs.start_index, 100);
    let ids: Vec<&str> = songs.items.iter().map(|song| song.id.as_str()).collect();
    assert_eq!(ids, ["s101", "s102"]);
}

#[tokio::test]
async fn playlist_header_reads_count_duration_and_comment() {
    let mock = MockServer::start().await;
    mount_playlist_summary(&mock).await;

    let playlist = server(&mock).playlist("p1").await.unwrap();
    assert_eq!(playlist.name, "Playlist p1");
    assert_eq!(playlist.song_count, 2);
    assert_eq!(playlist.duration_ms, 491_000);
    assert_eq!(playlist.comment.as_deref(), Some("Mix"));
}
