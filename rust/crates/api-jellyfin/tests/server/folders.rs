use feishin_api::{ApiError, MusicServer};
use feishin_core::model::FolderId;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{authed, items_path, list, page, server, song_json};

#[tokio::test]
async fn the_root_lists_the_music_libraries() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(items_path()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "Items": [
            { "Id": "m1", "Name": "Music", "CollectionType": "music" },
            { "Id": "v1", "Name": "Movies", "CollectionType": "movies" }
        ] })))
        .mount(&mock)
        .await;
    let jellyfin = server(&mock);

    let folders = jellyfin.subfolders(&FolderId::Root).await.unwrap();
    let songs = jellyfin
        .folder_song_page(&FolderId::Root, page(0, 50))
        .await
        .unwrap();

    assert_eq!(folders.len(), 1);
    assert_eq!(folders[0].id, "m1");
    assert_eq!(folders[0].name, "Music");
    assert!(songs.items.is_empty());
}

#[tokio::test]
async fn a_folder_lists_child_folders_by_parent_id() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(items_path()))
        .and(authed())
        .and(query_param("ParentId", "m1"))
        .and(query_param("IsFolder", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(
            vec![
                json!({ "Id": "f1", "Name": "Artist One", "Type": "Folder", "ImageTags": { "Primary": "t" } }),
                json!({ "Id": "f2", "Name": "Artist Two", "Type": "Folder" }),
            ],
            2,
        )))
        .expect(1)
        .mount(&mock)
        .await;

    let folders = server(&mock)
        .subfolders(&FolderId::directory("m1"))
        .await
        .unwrap();

    let names: Vec<&str> = folders.iter().map(|folder| folder.name.as_str()).collect();
    assert_eq!(names, ["Artist One", "Artist Two"]);
    assert_eq!(folders[0].cover_art_id.as_deref(), Some("f1"));
    assert_eq!(folders[1].cover_art_id, None);
}

#[tokio::test]
async fn folder_songs_are_paged_by_the_server() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(items_path()))
        .and(query_param("ParentId", "f1"))
        .and(query_param("IncludeItemTypes", "Audio"))
        .and(query_param("StartIndex", "100"))
        .and(query_param("Limit", "100"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(list(vec![song_json("s101", "al1")], 101)),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let songs = server(&mock)
        .folder_song_page(&FolderId::directory("f1"), page(100, 100))
        .await
        .unwrap();

    assert_eq!(songs.total_count, Some(101));
    assert_eq!(songs.start_index, 100);
    assert_eq!(songs.items[0].id, "s101");
}

#[tokio::test]
async fn sharing_is_not_available() {
    let mock = MockServer::start().await;
    let jellyfin = server(&mock);

    let refused = jellyfin.create_share(&["s1".to_owned()], None).await;

    assert!(!jellyfin.can_share());
    assert!(matches!(refused, Err(ApiError::Unsupported(_))));
}
