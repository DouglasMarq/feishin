use feishin_api::MusicServer;
use feishin_core::model::{AlbumListQuery, Paging, SongListQuery};
use serde_json::{Value, json};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{mount_ping, native_song_json, server, subsonic_ok};

async fn mount_year(mock: &MockServer, year: &str, songs: Vec<Value>) {
    let total = songs.len().to_string();
    Mock::given(method("GET"))
        .and(path("/api/song"))
        .and(query_param("year", year))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-total-count", total.as_str())
                .set_body_json(json!(songs)),
        )
        .mount(mock)
        .await;
}

async fn mount_two_years(mock: &MockServer) {
    mount_ping(mock, "0.58.0", &[]).await;
    mount_year(
        mock,
        "2021",
        vec![native_song_json("s1", "One"), native_song_json("s2", "Two")],
    )
    .await;
    mount_year(mock, "2022", vec![native_song_json("s3", "Three")]).await;
}

fn twenty_twenty_one_to_two() -> SongListQuery {
    SongListQuery {
        year_from: Some(2021),
        year_to: Some(2022),
        ..SongListQuery::default()
    }
}

#[tokio::test]
async fn a_year_range_lists_and_counts_every_year_in_it() {
    let mock = MockServer::start().await;
    mount_two_years(&mock).await;

    let server = server(&mock, Some("jwt-1"));
    let query = twenty_twenty_one_to_two();
    let everything = Paging {
        start_index: 0,
        limit: 50,
    };
    let page = server.song_list(&query, everything).await.expect("songs");
    let ids: Vec<&str> = page.items.iter().map(|song| song.id.as_str()).collect();
    assert_eq!(ids, ["s1", "s2", "s3"]);
    assert_eq!(page.total_count, Some(3));
    assert_eq!(server.song_count(&query).await.expect("count"), 3);
}

#[tokio::test]
async fn a_page_inside_a_year_range_spans_the_year_boundary() {
    let mock = MockServer::start().await;
    mount_two_years(&mock).await;

    let server = server(&mock, Some("jwt-1"));
    let window = Paging {
        start_index: 1,
        limit: 2,
    };
    let page = server
        .song_list(&twenty_twenty_one_to_two(), window)
        .await
        .expect("songs");
    assert_eq!(page.total_count, Some(3));
    assert_eq!(page.start_index, 1);
    let requests = mock.received_requests().await.unwrap_or_default();
    let fetched_window = |year: &str, window: &str| {
        requests.iter().any(|request| {
            request.url.path() == "/api/song"
                && request
                    .url
                    .query()
                    .is_some_and(|query| query.contains(year) && query.contains(window))
        })
    };
    assert!(fetched_window("year=2021", "_start=1&_end=2"));
    assert!(fetched_window("year=2022", "_start=0&_end=1"));
}

fn subsonic_album(id: &str, name: &str, year: i32) -> Value {
    json!({ "id": id, "name": name, "year": year, "songCount": 1, "duration": 60 })
}

#[tokio::test]
async fn an_album_year_range_uses_the_subsonic_year_list() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.58.0", &[]).await;
    Mock::given(method("GET"))
        .and(path("/rest/getAlbumList2.view"))
        .and(query_param("type", "byYear"))
        .and(query_param("fromYear", "2020"))
        .and(query_param("toYear", "2022"))
        .respond_with(subsonic_ok(json!({ "albumList2": { "album": [
            subsonic_album("al1", "Second Wave", 2021),
            subsonic_album("al2", "Afterglow", 2022)
        ] } })))
        .mount(&mock)
        .await;

    let server = server(&mock, Some("jwt-1"));
    let query = AlbumListQuery {
        year_from: Some(2020),
        year_to: Some(2022),
        ..AlbumListQuery::default()
    };
    let page = server
        .album_list(
            &query,
            Paging {
                start_index: 0,
                limit: 10,
            },
        )
        .await
        .expect("albums");
    let names: Vec<&str> = page.items.iter().map(|album| album.name.as_str()).collect();
    assert_eq!(names, ["Second Wave", "Afterglow"]);
    assert_eq!(server.album_count(&query).await.expect("count"), 2);
}
