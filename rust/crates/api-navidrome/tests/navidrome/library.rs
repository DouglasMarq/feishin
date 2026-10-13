use feishin_api::MusicServer;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{mount_ping, server, subsonic_ok};

#[tokio::test]
async fn genres_use_tag_endpoint_on_new_servers() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.58.0", &[]).await;
    Mock::given(method("GET"))
        .and(path("/api/tag"))
        .and(query_param("tag_name", "genre"))
        .and(query_param("_sort", "tagValue"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "id": "t1", "tagName": "genre", "tagValue": "Jazz", "albumCount": 2, "songCount": 20 }
        ])))
        .expect(1)
        .mount(&mock)
        .await;

    let genres = server(&mock, Some("jwt-1"))
        .genre_list()
        .await
        .expect("genres");
    assert_eq!(genres[0].id, "t1");
    assert_eq!(genres[0].name, "Jazz");
    assert_eq!(genres[0].song_count, Some(20));
}

#[tokio::test]
async fn random_songs_send_the_genre_name_for_a_genre_id() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.58.0", &[]).await;
    Mock::given(method("GET"))
        .and(path("/api/tag"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "id": "t-rock", "tagName": "genre", "tagValue": "Rock", "albumCount": 1, "songCount": 2 }
        ])))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/getRandomSongs.view"))
        .and(query_param("genre", "Rock"))
        .respond_with(subsonic_ok(json!({ "randomSongs": { "song": [] } })))
        .expect(1)
        .mount(&mock)
        .await;

    let random = server(&mock, Some("jwt-1"))
        .random_songs(5, Some("t-rock"))
        .await
        .expect("random songs");
    assert!(random.is_empty());
}
