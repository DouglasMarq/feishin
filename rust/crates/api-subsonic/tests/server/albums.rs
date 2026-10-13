use feishin_api::MusicServer;
use feishin_core::model::{AlbumListQuery, AlbumSort};
use serde_json::{Value, json};
use wiremock::matchers::{path, query_param};
use wiremock::{Mock, MockServer, Request};

use crate::support::{album, offset_response, ok, paging, server, song};

async fn mount_sorted_and_genre_album_lists(mock: &MockServer) {
    Mock::given(path("/rest/getAlbumList2.view"))
        .and(query_param("type", "newest"))
        .and(query_param("size", "2"))
        .and(query_param("offset", "4"))
        .respond_with(
            ok(json!({ "albumList2": { "album": [album("a1", "One"), album("a2", "Two")] } }))
                .insert_header("x-total-count", "57"),
        )
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(path("/rest/getAlbumList2.view"))
        .and(query_param("type", "byGenre"))
        .and(query_param("genre", "Rock"))
        .respond_with(ok(
            json!({ "albumList2": { "album": [album("a3", "Three")] } }),
        ))
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn album_list_maps_sort_and_reads_total_header() {
    let mock = MockServer::start().await;
    mount_sorted_and_genre_album_lists(&mock).await;
    let server = server(&mock);

    let query = AlbumListQuery {
        sort: AlbumSort::RecentlyAdded,
        ..Default::default()
    };
    let page = server
        .album_list(&query, paging(4, 2))
        .await
        .expect("albums");
    assert_eq!(page.total_count, Some(57));
    assert_eq!(page.start_index, 4);
    let first = &page.items[0];
    assert_eq!(first.id, "a1");
    assert_eq!(first.name, "One");
    assert_eq!(first.song_count, 10);
    assert_eq!(first.duration_ms, 1_800_000);
    assert_eq!(first.year, Some(2020));
    assert_eq!(first.album_artists[0].id, "ar-1");
    assert_eq!(first.cover_art_id.as_deref(), Some("al-a1"));

    let genre = AlbumListQuery {
        genre_ids: vec!["Rock".into()],
        ..Default::default()
    };
    let page = server
        .album_list(&genre, paging(0, 50))
        .await
        .expect("genre");
    assert_eq!(page.items[0].id, "a3");
    assert_eq!(page.total_count, None);
}

async fn mount_starred_and_searched_albums(mock: &MockServer) {
    let mut favorite = album("a2", "Beta");
    favorite["starred"] = json!("2024-01-01T00:00:00Z");
    Mock::given(path("/rest/getStarred2.view"))
        .respond_with(ok(json!({
            "starred2": { "album": [album("a1", "gamma"), favorite, album("a3", "Alpha")] }
        })))
        .mount(mock)
        .await;
    Mock::given(path("/rest/search3.view"))
        .and(query_param("query", "be"))
        .and(query_param("albumCount", "5"))
        .and(query_param("albumOffset", "0"))
        .and(query_param("songCount", "0"))
        .respond_with(ok(
            json!({ "searchResult3": { "album": [album("a2", "Beta")] } }),
        ))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(path("/rest/search3.view"))
        .and(query_param("query", "be"))
        .and(query_param("albumCount", "500"))
        .and(query_param("albumOffset", "0"))
        .respond_with(ok(
            json!({ "searchResult3": { "album": [album("a2", "Beta")] } }),
        ))
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn album_list_favorites_and_search() {
    let mock = MockServer::start().await;
    mount_starred_and_searched_albums(&mock).await;
    let server = server(&mock);

    let query = AlbumListQuery {
        favorite: Some(true),
        sort: AlbumSort::Name,
        ..Default::default()
    };
    let page = server
        .album_list(&query, paging(0, 2))
        .await
        .expect("favorites");
    let names: Vec<&str> = page.items.iter().map(|album| album.name.as_str()).collect();
    assert_eq!(names, ["Alpha", "Beta"]);
    assert_eq!(page.total_count, Some(3));
    assert!(page.items[1].favorite);
    assert_eq!(server.album_count(&query).await.ok(), Some(3));

    let search = AlbumListQuery {
        search: Some("be".into()),
        ..Default::default()
    };
    let page = server
        .album_list(&search, paging(0, 5))
        .await
        .expect("search");
    assert_eq!(page.items.len(), 1);
    assert_eq!(server.album_count(&search).await.ok(), Some(1));
}

#[tokio::test]
async fn album_count_pages_without_total_header() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/getAlbumList2.view"))
        .and(query_param("type", "alphabeticalByName"))
        .and(query_param("size", "500"))
        .respond_with(|request: &Request| {
            let albums: Vec<Value> = offset_response(request, 1_203, "offset", "size")
                .iter()
                .map(|_| album("a", "x"))
                .collect();
            ok(json!({ "albumList2": { "album": albums } }))
        })
        .expect(3)
        .mount(&mock)
        .await;
    assert_eq!(
        server(&mock)
            .album_count(&AlbumListQuery::default())
            .await
            .ok(),
        Some(1_203)
    );
}

#[tokio::test]
async fn album_detail_includes_songs() {
    let mock = MockServer::start().await;
    let mut detail = album("a1", "One");
    detail["song"] = json!([song("s1", "First"), song("s2", "Second")]);
    detail["releaseDate"] = json!({ "year": 2019, "month": 5, "day": 3 });
    detail["userRating"] = json!(4);
    Mock::given(path("/rest/getAlbum.view"))
        .and(query_param("id", "a1"))
        .respond_with(ok(json!({ "album": detail })))
        .mount(&mock)
        .await;

    let detail = server(&mock).album("a1").await.expect("album");
    assert_eq!(detail.album.year, Some(2019));
    assert_eq!(detail.album.user_rating, Some(4));
    assert_eq!(detail.songs.len(), 2);
    assert_eq!(detail.songs[1].title, "Second");
}
