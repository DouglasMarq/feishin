use feishin_api::MusicServer;
use feishin_core::model::{FolderId, Paging};
use serde_json::{Value, json};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer};

use crate::support::{server, subsonic_ok};

fn song_child(id: &str) -> Value {
    json!({
        "id": id, "isDir": false, "title": format!("Song {id}"), "album": "Album One",
        "albumId": "al1", "artist": "Artist One", "artistId": "ar1", "duration": 200,
        "track": 1, "path": format!("Artist One/Album One/{id}.flac")
    })
}

fn paging(start_index: usize, limit: usize) -> Paging {
    Paging { start_index, limit }
}

#[tokio::test]
async fn the_root_lists_the_index_folders() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/getIndexes.view"))
        .respond_with(subsonic_ok(json!({
            "indexes": { "index": [
                { "name": "A", "artist": [{ "id": "f1", "name": "Artist One" }] },
                { "name": "Z", "artist": [{ "id": "f2", "name": "Zed", "coverArt": "ar-f2" }] }
            ] }
        })))
        .expect(1)
        .mount(&mock)
        .await;
    let navidrome = server(&mock, Some("jwt-1"));

    let folders = navidrome.subfolders(&FolderId::Root).await.unwrap();
    let songs = navidrome
        .folder_song_page(&FolderId::Root, paging(0, 100))
        .await
        .unwrap();

    let ids: Vec<&str> = folders.iter().map(|folder| folder.id.as_str()).collect();
    assert_eq!(ids, ["f1", "f2"]);
    assert_eq!(folders[1].cover_art_id.as_deref(), Some("ar-f2"));
    assert_eq!(songs.total_count, Some(0));
}

#[tokio::test]
async fn a_directory_lists_subfolders_and_pages_songs() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/getMusicDirectory.view"))
        .and(query_param("id", "f1"))
        .respond_with(subsonic_ok(json!({
            "directory": { "id": "f1", "name": "Artist One", "child": [
                { "id": "f1a", "isDir": true, "title": "Album One" },
                song_child("s1"),
                song_child("s2")
            ] }
        })))
        .expect(1)
        .mount(&mock)
        .await;
    let navidrome = server(&mock, Some("jwt-1"));
    let folder = FolderId::directory("f1");

    let folders = navidrome.subfolders(&folder).await.unwrap();
    let songs = navidrome
        .folder_song_page(&folder, paging(1, 10))
        .await
        .unwrap();

    assert_eq!(folders.len(), 1);
    assert_eq!(folders[0].name, "Album One");
    assert_eq!(songs.total_count, Some(2));
    assert_eq!(songs.items.len(), 1);
    assert_eq!(songs.items[0].id, "s2");
    assert_eq!(songs.items[0].duration_ms, 200_000);
}
