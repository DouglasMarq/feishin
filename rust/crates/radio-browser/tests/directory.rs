use feishin_radio_browser::{DirectoryError, RadioBrowser, StationQuery, USER_AGENT};
use serde_json::{Value, json};
use wiremock::matchers::{header, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn station_fixture() -> Value {
    json!([
        {
            "changeuuid": "c1",
            "stationuuid": "9617a958-0601-11e8-ae97-52543be04c81",
            "name": "Smooth Jazz Radio",
            "url": "http://jazz.example.com/listen.pls",
            "url_resolved": "http://jazz.example.com/stream.mp3",
            "homepage": "https://jazz.example.com/",
            "favicon": "https://jazz.example.com/icon.png",
            "tags": "jazz,smooth jazz",
            "country": "Germany",
            "countrycode": "DE",
            "state": "",
            "language": "german",
            "votes": 1234,
            "codec": "MP3",
            "bitrate": 128,
            "lastcheckok": 1,
            "clickcount": 77
        },
        {
            "stationuuid": "broken",
            "name": "No Address",
            "url": "",
            "url_resolved": "",
            "tags": ""
        }
    ])
}

async fn mount_servers(mock: &MockServer, names: Value) {
    Mock::given(method("GET"))
        .and(path("/json/servers"))
        .and(header("user-agent", USER_AGENT))
        .respond_with(ResponseTemplate::new(200).set_body_json(names))
        .expect(1)
        .mount(mock)
        .await;
}

fn mock_host(mock: &MockServer) -> String {
    mock.uri().trim_start_matches("http://").to_owned()
}

fn jazz_in_germany() -> StationQuery {
    StationQuery {
        name: "jazz".into(),
        country: "Germany".into(),
        ..StationQuery::default()
    }
}

#[tokio::test]
async fn search_resolves_a_mirror_and_decodes_stations() {
    let mock = MockServer::start().await;
    mount_servers(
        &mock,
        json!([{ "ip": "127.0.0.1", "name": mock_host(&mock) }]),
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/json/stations/search"))
        .and(header("user-agent", USER_AGENT))
        .and(query_param("name", "jazz"))
        .and(query_param("country", "Germany"))
        .and(query_param_is_missing("tag"))
        .and(query_param("hidebroken", "true"))
        .and(query_param("offset", "0"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_json(station_fixture()))
        .expect(2)
        .mount(&mock)
        .await;
    let directory = RadioBrowser::with_discovery_url(&mock.uri()).expect("client");

    let stations = directory.search(&jazz_in_germany()).await.expect("search");
    directory
        .search(&jazz_in_germany())
        .await
        .expect("second search");

    assert_eq!(stations.len(), 1);
    let station = &stations[0];
    assert_eq!(station.uuid, "9617a958-0601-11e8-ae97-52543be04c81");
    assert_eq!(station.name, "Smooth Jazz Radio");
    assert_eq!(station.stream_url, "http://jazz.example.com/stream.mp3");
    assert_eq!(
        station.homepage_url.as_deref(),
        Some("https://jazz.example.com/")
    );
    assert_eq!(station.tags, ["jazz", "smooth jazz"]);
    assert_eq!(station.country_code, "DE");
    assert_eq!(station.codec, "MP3");
    assert_eq!(station.bitrate_kbps, Some(128));
    assert_eq!(station.votes, 1234);
}

#[tokio::test]
async fn a_failed_mirror_lookup_falls_back_to_the_discovery_host() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/json/servers"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/json/stations/search"))
        .and(query_param("tag", "rock"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(1)
        .mount(&mock)
        .await;
    let directory = RadioBrowser::with_discovery_url(&mock.uri()).expect("client");
    let query = StationQuery {
        tag: "rock".into(),
        ..StationQuery::default()
    };

    assert!(directory.search(&query).await.expect("search").is_empty());
}

#[tokio::test]
async fn a_failing_mirror_is_resolved_again_and_retried_once() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/json/servers"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!([{ "name": mock_host(&mock) }])),
        )
        .expect(2)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/json/stations/search"))
        .respond_with(ResponseTemplate::new(502))
        .up_to_n_times(1)
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/json/stations/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(station_fixture()))
        .expect(1)
        .mount(&mock)
        .await;
    let directory = RadioBrowser::with_discovery_url(&mock.uri()).expect("client");

    let stations = directory.search(&jazz_in_germany()).await.expect("search");

    assert_eq!(stations.len(), 1);
}

#[tokio::test]
async fn server_errors_are_reported() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/json/servers"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/json/stations/search"))
        .respond_with(ResponseTemplate::new(500))
        .expect(2)
        .mount(&mock)
        .await;
    let directory = RadioBrowser::with_discovery_url(&mock.uri()).expect("client");

    let failed = directory.search(&jazz_in_germany()).await;

    assert!(matches!(failed, Err(DirectoryError::Status(500))));
}

#[tokio::test]
async fn malformed_answers_are_decode_errors() {
    let mock = MockServer::start().await;
    mount_servers(&mock, json!([])).await;
    Mock::given(method("GET"))
        .and(path("/json/stations/search"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>"))
        .mount(&mock)
        .await;
    let directory = RadioBrowser::with_discovery_url(&mock.uri()).expect("client");

    let failed = directory.search(&jazz_in_germany()).await;

    assert!(matches!(failed, Err(DirectoryError::Decode(_))));
}

#[tokio::test]
async fn clicks_are_counted_for_the_station() {
    let mock = MockServer::start().await;
    mount_servers(&mock, json!([{ "name": mock_host(&mock) }])).await;
    Mock::given(method("GET"))
        .and(path("/json/url/9617a958"))
        .and(header("user-agent", USER_AGENT))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ok": true, "message": "retrieved station url", "name": "Smooth Jazz Radio",
            "url": "http://jazz.example.com/stream.mp3"
        })))
        .expect(1)
        .mount(&mock)
        .await;
    let directory = RadioBrowser::with_discovery_url(&mock.uri()).expect("client");

    directory.count_click("9617a958").await.expect("click");
}
