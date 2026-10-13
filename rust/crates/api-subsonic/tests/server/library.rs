use feishin_api::MusicServer;
use serde_json::json;
use wiremock::matchers::path;
use wiremock::{Mock, MockServer};

use crate::support::{ok, server};

#[tokio::test]
async fn genres_and_music_folders() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/getGenres.view"))
        .respond_with(ok(json!({
            "genres": { "genre": [
                { "value": "rock", "songCount": 10, "albumCount": 2 },
                { "value": "Ambient", "songCount": 3, "albumCount": 1 }
            ] }
        })))
        .mount(&mock)
        .await;
    Mock::given(path("/rest/getMusicFolders.view"))
        .respond_with(ok(json!({
            "musicFolders": { "musicFolder": [{ "id": 1, "name": "Music" }] }
        })))
        .mount(&mock)
        .await;
    let server = server(&mock);

    let genres = server.genre_list().await.expect("genres");
    assert_eq!(genres[0].name, "Ambient");
    assert_eq!(genres[1].id, "rock");
    assert_eq!(genres[1].song_count, Some(10));
    let folders = server.music_folders().await.expect("folders");
    assert_eq!(folders[0].id, "1");
    assert_eq!(folders[0].name, "Music");
}
