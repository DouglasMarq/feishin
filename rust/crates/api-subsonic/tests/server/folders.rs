use feishin_api::MusicServer;
use feishin_core::model::FolderId;
use serde_json::{Value, json};
use wiremock::matchers::{path, query_param};
use wiremock::{Mock, MockServer};

use crate::support::{ok, paging, server, song};

fn folder_child(id: &str, title: &str) -> Value {
    json!({ "id": id, "parent": "root", "isDir": true, "title": title, "coverArt": format!("c-{id}") })
}

fn song_child(id: &str) -> Value {
    let mut child = song(id, &format!("Song {id}"));
    if let Some(fields) = child.as_object_mut() {
        fields.insert("isDir".into(), json!(false));
    }
    child
}

async fn mount_directory(mock: &MockServer, id: &str, children: Vec<Value>, expected_calls: u64) {
    Mock::given(path("/rest/getMusicDirectory.view"))
        .and(query_param("id", id))
        .respond_with(ok(json!({
            "directory": { "id": id, "name": "Folder", "child": children }
        })))
        .expect(expected_calls)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn the_root_lists_indexed_artist_folders_and_loose_songs() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/getIndexes.view"))
        .respond_with(ok(json!({
            "indexes": {
                "ignoredArticles": "The",
                "index": [
                    { "name": "A", "artist": [{ "id": 1, "name": "ABBA" }] },
                    { "name": "B", "artist": [{ "id": "2", "name": "Bach" }] }
                ],
                "child": [song_child("loose-1")]
            }
        })))
        .expect(1)
        .mount(&mock)
        .await;
    let server = server(&mock);

    let folders = server.subfolders(&FolderId::Root).await.unwrap();
    let songs = server
        .folder_song_page(&FolderId::Root, paging(0, 50))
        .await
        .unwrap();

    let names: Vec<&str> = folders.iter().map(|folder| folder.name.as_str()).collect();
    assert_eq!(names, ["ABBA", "Bach"]);
    assert_eq!(folders[0].id, "1");
    assert_eq!(songs.total_count, Some(1));
    assert_eq!(songs.items[0].id, "loose-1");
}

#[tokio::test]
async fn a_directory_splits_folders_from_songs_and_pages_its_songs() {
    let mock = MockServer::start().await;
    let children = vec![
        folder_child("d1", "Disc 1"),
        song_child("s1"),
        song_child("s2"),
        folder_child("d2", "Disc 2"),
        song_child("s3"),
    ];
    mount_directory(&mock, "artist-1", children, 1).await;
    let server = server(&mock);
    let folder = FolderId::directory("artist-1");

    let folders = server.subfolders(&folder).await.unwrap();
    let first = server
        .folder_song_page(&folder, paging(0, 2))
        .await
        .unwrap();
    let second = server
        .folder_song_page(&folder, paging(2, 2))
        .await
        .unwrap();

    let folder_ids: Vec<&str> = folders.iter().map(|folder| folder.id.as_str()).collect();
    assert_eq!(folder_ids, ["d1", "d2"]);
    assert_eq!(folders[1].cover_art_id.as_deref(), Some("c-d2"));
    let first_ids: Vec<&str> = first.items.iter().map(|song| song.id.as_str()).collect();
    assert_eq!(first_ids, ["s1", "s2"]);
    assert_eq!(first.total_count, Some(3));
    assert_eq!(second.start_index, 2);
    assert_eq!(second.items.len(), 1);
}

#[tokio::test]
async fn only_the_latest_directory_stays_cached() {
    let mock = MockServer::start().await;
    mount_directory(&mock, "a", vec![song_child("a1")], 2).await;
    mount_directory(&mock, "b", vec![song_child("b1")], 1).await;
    let server = server(&mock);
    let (first, second) = (FolderId::directory("a"), FolderId::directory("b"));

    server.subfolders(&first).await.unwrap();
    server
        .folder_song_page(&first, paging(0, 10))
        .await
        .unwrap();
    server.subfolders(&second).await.unwrap();
    let again = server
        .folder_song_page(&first, paging(0, 10))
        .await
        .unwrap();

    assert_eq!(again.items[0].id, "a1");
}

#[tokio::test]
async fn opening_a_folder_lists_it_once_for_folders_and_songs_together() {
    let mock = MockServer::start().await;
    mount_directory(&mock, "album-1", vec![song_child("t1")], 1).await;
    let server = server(&mock);
    let folder = FolderId::directory("album-1");

    let (folders, songs) = tokio::join!(
        server.subfolders(&folder),
        server.folder_song_page(&folder, paging(0, 10))
    );

    assert!(folders.unwrap().is_empty());
    assert_eq!(songs.unwrap().items[0].id, "t1");
}
