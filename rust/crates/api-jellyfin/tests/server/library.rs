use feishin_api::MusicServer;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{USER, album_json, artist_json, items_path, list, server, song_json};

#[tokio::test]
async fn genres_and_music_folders() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/musicgenres"))
        .and(query_param("UserId", USER))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(
            vec![json!({ "Id": "g1", "Name": "Rock", "ImageTags": {}, "ImageBlurHashes": {} })],
            1,
        )))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path(items_path()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "Items": [
            { "Id": "m1", "Name": "Music", "CollectionType": "music" },
            { "Id": "v1", "Name": "Movies", "CollectionType": "movies" },
            { "Id": "b1", "Name": "Books", "CollectionType": "books" }
        ] })))
        .mount(&mock)
        .await;

    let jellyfin = server(&mock);
    let genres = jellyfin.genre_list().await.unwrap();
    assert_eq!(genres[0].name, "Rock");
    let folders = jellyfin.music_folders().await.unwrap();
    assert_eq!(folders.len(), 1);
    assert_eq!(folders[0].id, "m1");
}

async fn mount_search_results(mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path(items_path()))
        .and(query_param("IncludeItemTypes", "MusicAlbum"))
        .and(query_param("SearchTerm", "abc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(vec![album_json("al1")], 1)))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/artists/albumArtists"))
        .and(query_param("SearchTerm", "abc"))
        .and(query_param("IncludeArtists", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(vec![artist_json("ar1")], 1)))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path(items_path()))
        .and(query_param("IncludeItemTypes", "Audio"))
        .and(query_param("SearchTerm", "abc"))
        .and(query_param("Limit", "7"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(vec![song_json("s1", "a")], 1)))
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn search_queries_albums_artists_and_songs() {
    let mock = MockServer::start().await;
    mount_search_results(&mock).await;

    let results = server(&mock).search("abc", 7).await.unwrap();
    assert_eq!(results.albums.len(), 1);
    assert_eq!(results.artists.len(), 1);
    assert_eq!(results.songs.len(), 1);
}
