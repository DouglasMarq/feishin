use feishin_api::MusicServer;
use feishin_core::model::{SongListQuery, SongSort, SortOrder};
use serde_json::{Value, json};
use wiremock::matchers::{path, query_param};
use wiremock::{Mock, MockServer, Request};

use crate::support::{album, offset_response, ok, paging, server, song};

#[tokio::test]
async fn song_list_without_filters_uses_empty_search3() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/search3.view"))
        .and(query_param("query", ""))
        .and(query_param("songCount", "2"))
        .and(query_param("songOffset", "100"))
        .and(query_param("albumCount", "0"))
        .and(query_param("artistCount", "0"))
        .and(query_param("musicFolderId", "mf-1"))
        .respond_with(ok(json!({
            "searchResult3": { "song": [song("s1", "One"), song("s2", "Two")] }
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let query = SongListQuery {
        music_folder_id: Some("mf-1".into()),
        ..Default::default()
    };
    let page = server(&mock)
        .song_list(&query, paging(100, 2))
        .await
        .expect("song list");
    assert_eq!(page.start_index, 100);
    assert_eq!(page.total_count, None);
    assert_eq!(page.items.len(), 2);
    let first = &page.items[0];
    assert_eq!(first.id, "s1");
    assert_eq!(first.server_id, "srv-1");
    assert_eq!(first.title, "One");
    assert_eq!(first.album.as_deref(), Some("Album"));
    assert_eq!(first.album_id.as_deref(), Some("al-1"));
    assert_eq!(first.artists[0].name, "Artist");
    assert_eq!(first.duration_ms, 180_000);
    assert_eq!(first.bitrate_kbps, Some(320));
    assert_eq!(first.container.as_deref(), Some("mpeg"));
    assert_eq!(first.cover_art_id.as_deref(), Some("mf-s1"));
    assert_eq!(first.play_count, 3);
    assert!(!first.favorite);
}

async fn mount_search_genre_and_random(mock: &MockServer) {
    Mock::given(path("/rest/search3.view"))
        .and(query_param("query", "love"))
        .and(query_param("songCount", "10"))
        .and(query_param("songOffset", "20"))
        .respond_with(ok(
            json!({ "searchResult3": { "song": [song("s1", "Love")] } }),
        ))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(path("/rest/getSongsByGenre.view"))
        .and(query_param("genre", "Rock"))
        .and(query_param("count", "10"))
        .and(query_param("offset", "20"))
        .respond_with(ok(
            json!({ "songsByGenre": { "song": [song("s2", "Rock")] } }),
        ))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(path("/rest/getRandomSongs.view"))
        .and(query_param("size", "10"))
        .and(query_param("genre", "Jazz"))
        .and(query_param("fromYear", "1990"))
        .respond_with(ok(
            json!({ "randomSongs": { "song": [song("s3", "Rand")] } }),
        ))
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn song_list_search_genre_and_random() {
    let mock = MockServer::start().await;
    mount_search_genre_and_random(&mock).await;
    let server = server(&mock);

    let search = SongListQuery {
        search: Some("love".into()),
        genre_ids: vec!["Rock".into()],
        ..Default::default()
    };
    let page = server
        .song_list(&search, paging(20, 10))
        .await
        .expect("search");
    assert_eq!(page.items[0].id, "s1");

    let genre = SongListQuery {
        genre_ids: vec!["Rock".into()],
        ..Default::default()
    };
    let page = server
        .song_list(&genre, paging(20, 10))
        .await
        .expect("genre");
    assert_eq!(page.items[0].id, "s2");
    assert_eq!(page.start_index, 20);

    let random = SongListQuery {
        sort: SongSort::Random,
        genre_ids: vec!["Jazz".into()],
        year_from: Some(1990),
        ..Default::default()
    };
    let page = server
        .song_list(&random, paging(0, 10))
        .await
        .expect("random");
    assert_eq!(page.items[0].id, "s3");
}

fn starred_song(id: &str, title: &str) -> Value {
    let mut starred = song(id, title);
    starred["starred"] = json!("2024-06-01T00:00:00Z");
    starred
}

#[tokio::test]
async fn song_list_favorites_are_filtered_sorted_and_paged() {
    let mock = MockServer::start().await;
    let mut other = song("s4", "Other");
    other["albumArtists"] = json!([{ "id": "ar-9", "name": "Nobody" }]);
    Mock::given(path("/rest/getStarred2.view"))
        .respond_with(ok(json!({
            "starred2": {
                "song": [starred_song("s1", "charlie"), starred_song("s2", "Alpha"), starred_song("s3", "bravo"), other],
                "album": [], "artist": []
            }
        })))
        .expect(1)
        .mount(&mock)
        .await;
    let server = server(&mock);

    let query = SongListQuery {
        favorite: Some(true),
        sort: SongSort::Name,
        order: SortOrder::Desc,
        artist_ids: vec!["ar-1".into()],
        ..Default::default()
    };
    let page = server
        .song_list(&query, paging(1, 1))
        .await
        .expect("favorites");
    assert_eq!(page.total_count, Some(3));
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].title, "bravo");
    assert!(page.items[0].favorite);
    assert_eq!(server.song_count(&query).await.ok(), Some(3));
}

async fn mount_albums_and_artist(mock: &MockServer) {
    Mock::given(path("/rest/getArtist.view"))
        .and(query_param("id", "ar-1"))
        .respond_with(ok(json!({
            "artist": { "id": "ar-1", "name": "Artist", "albumCount": 1, "album": [album("al-2", "Second")] }
        })))
        .expect(2)
        .mount(mock)
        .await;
    let mut first = album("al-1", "First");
    first["song"] = json!([song("s1", "One"), song("s2", "Two")]);
    Mock::given(path("/rest/getAlbum.view"))
        .and(query_param("id", "al-1"))
        .respond_with(ok(json!({ "album": first })))
        .mount(mock)
        .await;
    let mut second = album("al-2", "Second");
    second["song"] = json!([song("s3", "Three")]);
    Mock::given(path("/rest/getAlbum.view"))
        .and(query_param("id", "al-2"))
        .respond_with(ok(json!({ "album": second })))
        .mount(mock)
        .await;
}

#[tokio::test]
async fn song_list_by_album_and_artist_ids() {
    let mock = MockServer::start().await;
    mount_albums_and_artist(&mock).await;
    let server = server(&mock);

    let query = SongListQuery {
        album_ids: vec!["al-1".into()],
        artist_ids: vec!["ar-1".into()],
        ..Default::default()
    };
    let page = server
        .song_list(&query, paging(0, 0))
        .await
        .expect("by ids");
    let ids: Vec<&str> = page.items.iter().map(|song| song.id.as_str()).collect();
    assert_eq!(ids, ["s1", "s2", "s3"]);
    assert_eq!(page.total_count, Some(3));
    assert_eq!(server.song_count(&query).await.ok(), Some(3));
}

async fn mount_counted_song_sources(mock: &MockServer) {
    Mock::given(path("/rest/search3.view"))
        .and(query_param("query", ""))
        .respond_with(|request: &Request| {
            let songs = offset_response(request, 12_345, "songOffset", "songCount");
            ok(json!({ "searchResult3": { "song": songs } }))
        })
        .expect(6 + 4 + 1)
        .mount(mock)
        .await;
    Mock::given(path("/rest/getSongsByGenre.view"))
        .and(query_param("genre", "Rock"))
        .respond_with(|request: &Request| {
            let songs = offset_response(request, 1_000, "offset", "count");
            ok(json!({ "songsByGenre": { "song": songs } }))
        })
        .mount(mock)
        .await;
    Mock::given(path("/rest/search3.view"))
        .and(query_param("query", "x"))
        .respond_with(|request: &Request| {
            let songs = offset_response(request, 742, "songOffset", "songCount");
            ok(json!({ "searchResult3": { "song": songs } }))
        })
        .mount(mock)
        .await;
}

#[tokio::test]
async fn song_count_probes_then_pages() {
    let mock = MockServer::start().await;
    mount_counted_song_sources(&mock).await;
    let server = server(&mock);

    assert_eq!(
        server.song_count(&SongListQuery::default()).await.ok(),
        Some(12_345)
    );
    let genre = SongListQuery {
        genre_ids: vec!["Rock".into()],
        ..Default::default()
    };
    assert_eq!(server.song_count(&genre).await.ok(), Some(1_000));
    let search = SongListQuery {
        search: Some("x".into()),
        ..Default::default()
    };
    assert_eq!(server.song_count(&search).await.ok(), Some(742));
}
