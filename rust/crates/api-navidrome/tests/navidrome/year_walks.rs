use feishin_api::MusicServer;
use feishin_core::model::SongListQuery;
use serde_json::{Value, json};
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{merged, mount_ping, native_song_json, server};

fn song_in_year(id: &str, year: i32) -> Value {
    merged(native_song_json(id, id), json!({ "year": year }))
}

fn songs_response(songs: Vec<Value>, total: usize) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .insert_header("x-total-count", total.to_string().as_str())
        .set_body_json(json!(songs))
}

async fn mount_year(mock: &MockServer, year: i32, songs: Vec<Value>) {
    let total = songs.len();
    Mock::given(method("GET"))
        .and(path("/api/song"))
        .and(query_param("year", year.to_string()))
        .respond_with(songs_response(songs, total))
        .mount(mock)
        .await;
}

async fn mount_sorted_by_year(mock: &MockServer, order: &str, start: usize, songs: Vec<Value>) {
    Mock::given(method("GET"))
        .and(path("/api/song"))
        .and(query_param_is_missing("year"))
        .and(query_param("_sort", "year"))
        .and(query_param("_order", order))
        .and(query_param("_start", start.to_string()))
        .respond_with(songs_response(songs, 3))
        .mount(mock)
        .await;
}

async fn mount_sparse_library(mock: &MockServer) {
    mount_ping(mock, "0.58.0", &[]).await;
    mount_year(mock, 0, Vec::new()).await;
    mount_year(mock, 1, vec![song_in_year("junk", 1)]).await;
    mount_year(
        mock,
        2020,
        vec![song_in_year("a", 2020), song_in_year("b", 2020)],
    )
    .await;
    mount_sorted_by_year(mock, "ASC", 0, vec![song_in_year("junk", 1)]).await;
    mount_sorted_by_year(mock, "ASC", 1, vec![song_in_year("a", 2020)]).await;
    mount_sorted_by_year(mock, "ASC", 3, vec![song_in_year("late", 2022)]).await;
    mount_sorted_by_year(mock, "DESC", 0, vec![song_in_year("late", 2022)]).await;
}

async fn song_requests(mock: &MockServer) -> usize {
    let requests = mock.received_requests().await.unwrap_or_default();
    requests
        .iter()
        .filter(|request| request.url.path() == "/api/song")
        .count()
}

#[tokio::test]
async fn a_wide_range_walks_only_the_years_that_have_songs() {
    let mock = MockServer::start().await;
    mount_sparse_library(&mock).await;

    let query = SongListQuery {
        year_to: Some(2021),
        ..SongListQuery::default()
    };
    let count = server(&mock, Some("jwt-1")).song_count(&query).await;
    assert_eq!(count.expect("count"), 3);
    assert!(song_requests(&mock).await <= 10);
}

#[tokio::test]
async fn an_open_range_without_dated_songs_is_empty_after_a_few_requests() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.58.0", &[]).await;
    Mock::given(method("GET"))
        .and(path("/api/song"))
        .respond_with(songs_response(Vec::new(), 0))
        .mount(&mock)
        .await;

    let query = SongListQuery {
        year_to: Some(2020),
        ..SongListQuery::default()
    };
    let count = server(&mock, Some("jwt-1")).song_count(&query).await;
    assert_eq!(count.expect("count"), 0);
    assert!(song_requests(&mock).await <= 3);
}
