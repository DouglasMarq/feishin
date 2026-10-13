use feishin_api::{ApiError, MusicServer};
use feishin_core::model::{Paging, Song, SongListQuery, SongSort, SortOrder};
use serde_json::json;
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{
    AudioFormat, mount_ping, native_song_json, query_values, server, subsonic_entry, subsonic_ok,
};

async fn mount_filtered_song_list(mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/song"))
        .and(query_param("_start", "20"))
        .and(query_param("_end", "70"))
        .and(query_param("_sort", "year"))
        .and(query_param("_order", "DESC"))
        .and(query_param("starred", "true"))
        .and(query_param("title", "love"))
        .and(query_param("missing", "false"))
        .and(query_param("library_id", "lib1"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-total-count", "1234")
                .set_body_json(json!([
                    native_song_json("s1", "Love One"),
                    native_song_json("s2", "Love Two")
                ])),
        )
        .expect(1)
        .mount(mock)
        .await;
}

fn filtered_song_query() -> SongListQuery {
    SongListQuery {
        sort: SongSort::Year,
        order: SortOrder::Desc,
        search: Some("love".into()),
        favorite: Some(true),
        genre_ids: vec!["g1".into(), "g2".into()],
        artist_ids: vec!["ar1".into()],
        music_folder_id: Some("lib1".into()),
        ..SongListQuery::default()
    }
}

fn assert_first_native_song(song: &Song) {
    assert_eq!(song.id, "s1");
    assert_eq!(song.server_id, "srv1");
    assert_eq!(song.duration_ms, 245_300);
    assert_eq!(song.artists[0].name, "Artist One");
    assert_eq!(song.album_id.as_deref(), Some("al1"));
    assert_eq!(song.user_rating, Some(4));
    assert!(song.favorite);
    assert_eq!(
        song.path.as_deref(),
        Some("/music/Artist One/Album One/01.flac")
    );
    assert_eq!(song.container.as_deref(), Some("flac"));
    assert_eq!(song.cover_art_id.as_deref(), Some("s1"));
    assert_eq!(
        song.replay_gain.and_then(|gain| gain.track_gain_db),
        Some(-7.1)
    );
}

async fn assert_repeated_filters_sent(mock: &MockServer) {
    let requests = mock.received_requests().await.unwrap_or_default();
    let list = requests
        .iter()
        .find(|request| request.url.path() == "/api/song")
        .expect("song list request");
    assert_eq!(query_values(list, "genre_id"), vec!["g1", "g2"]);
    assert_eq!(query_values(list, "artists_id"), vec!["ar1"]);
    assert!(query_values(list, "album_artist_id").is_empty());
}

#[tokio::test]
async fn song_list_sends_paging_sort_filters_and_reads_total() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.58.0 (b2ad9d0d)", &[]).await;
    mount_filtered_song_list(&mock).await;

    let paging = Paging {
        start_index: 20,
        limit: 50,
    };
    let page = server(&mock, Some("jwt-1"))
        .song_list(&filtered_song_query(), paging)
        .await
        .expect("song list");

    assert_eq!(page.start_index, 20);
    assert_eq!(page.total_count, Some(1234));
    assert_eq!(page.items.len(), 2);
    assert_first_native_song(&page.items[0]);
    assert_repeated_filters_sent(&mock).await;
}

#[tokio::test]
async fn song_count_on_old_server_uses_legacy_filters() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.54.5", &[]).await;
    Mock::given(method("GET"))
        .and(path("/api/song"))
        .and(query_param("_start", "0"))
        .and(query_param("_end", "1"))
        .and(query_param("_sort", "title"))
        .and(query_param("_order", "ASC"))
        .and(query_param("album_artist_id", "ar1"))
        .and(query_param_is_missing("missing"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-total-count", "42")
                .set_body_json(json!([native_song_json("s1", "One")])),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let server = server(&mock, Some("jwt-1"));
    let query = SongListQuery {
        artist_ids: vec!["ar1".into()],
        ..SongListQuery::default()
    };
    assert_eq!(server.song_count(&query).await.expect("count"), 42);
}

#[tokio::test]
async fn song_count_without_session_is_auth_error() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.58.0", &[]).await;
    let server = server(&mock, None);
    let result = server.song_count(&SongListQuery::default()).await;
    assert!(matches!(result, Err(ApiError::Auth(_))));
}

fn search_entry(id: &str) -> serde_json::Value {
    subsonic_entry(id, id, "p", AudioFormat::Mp3)
}

async fn mount_search(mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/search3.view"))
        .and(query_param("query", "abc"))
        .and(query_param("songCount", "5"))
        .respond_with(subsonic_ok(json!({
            "searchResult3": {
                "artist": [{ "id": "ar1", "name": "A", "albumCount": 2 }],
                "album": [{
                    "id": "al1", "name": "Al", "artist": "A", "artistId": "ar1", "coverArt": "al-1",
                    "songCount": 3, "duration": 100, "created": "c", "year": 1999
                }],
                "song": [search_entry("s1")]
            }
        })))
        .expect(1)
        .mount(mock)
        .await;
}

async fn mount_random_and_similar(mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/getRandomSongs.view"))
        .and(query_param("size", "2"))
        .and(query_param("genre", "Rock"))
        .respond_with(subsonic_ok(
            json!({ "randomSongs": { "song": [search_entry("r1"), search_entry("r2")] } }),
        ))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/getSimilarSongs.view"))
        .and(query_param("id", "s1"))
        .and(query_param("count", "3"))
        .respond_with(subsonic_ok(
            json!({ "similarSongs": { "song": [search_entry("s1"), search_entry("x1")] } }),
        ))
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn search_random_and_similar_use_subsonic() {
    let mock = MockServer::start().await;
    mount_search(&mock).await;
    mount_random_and_similar(&mock).await;

    let server = server(&mock, Some("jwt-1"));
    let results = server.search("abc", 5).await.expect("search");
    assert_eq!(results.artists[0].album_count, Some(2));
    assert_eq!(results.albums[0].album_artists[0].id, "ar1");
    assert_eq!(results.albums[0].year, Some(1999));
    assert_eq!(results.songs.len(), 1);
    let random = server.random_songs(2, Some("Rock")).await.expect("random");
    assert_eq!(random.len(), 2);
    let similar = server.similar_songs("s1", 3).await.expect("similar");
    assert_eq!(similar.len(), 1);
    assert_eq!(similar[0].id, "x1");
}

#[tokio::test]
async fn lyrics_from_opensubsonic_extension() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.58.0", &["songLyrics"]).await;
    Mock::given(method("GET"))
        .and(path("/api/song/s1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(native_song_json("s1", "One")))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/getLyricsBySongId.view"))
        .and(query_param("id", "s1"))
        .respond_with(subsonic_ok(json!({
            "lyricsList": { "structuredLyrics": [{
                "displayArtist": "Artist One",
                "displayTitle": "One",
                "lang": "eng",
                "synced": true,
                "line": [{ "start": 1500, "value": "hello" }]
            }]}
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let server = server(&mock, Some("jwt-1"));
    let song = server.song("s1").await.expect("song");
    let lyrics = server.lyrics(&song).await.expect("lyrics");
    assert_eq!(lyrics.as_deref(), Some("[00:01.50]hello"));
}
