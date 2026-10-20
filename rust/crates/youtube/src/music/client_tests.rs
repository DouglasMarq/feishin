use serde_json::json;
use wiremock::matchers::{body_partial_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;

const HOME: &str = include_str!("fixtures/home.json");
const CHARTS: &str = include_str!("fixtures/charts.json");
const ALBUM: &str = include_str!("fixtures/album.json");
const SEARCH: &str = include_str!("fixtures/search.json");
const EXPLORE: &str = include_str!("fixtures/explore.json");
const MOOD: &str = include_str!("fixtures/mood.json");
const NEXT: &str = include_str!("fixtures/next.json");
const COOKIES: &str = ".youtube.com\tTRUE\t/\tTRUE\t0\tSAPISID\tfixture-sapisid\n\
    .youtube.com\tTRUE\t/\tTRUE\t0\tSID\tfixture-sid\n";

fn locale() -> MusicLocale {
    MusicLocale {
        language: "pt".into(),
        country: "BR".into(),
    }
}

fn client(server: &MockServer) -> MusicClient {
    MusicClient::new(locale())
        .unwrap()
        .with_base_url(server.uri())
}

fn json_body(text: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_raw(text.as_bytes().to_vec(), "application/json")
}

async fn mount_browse(server: &MockServer, browse_body: serde_json::Value, response: &str) {
    Mock::given(method("POST"))
        .and(path("/browse"))
        .and(query_param("prettyPrint", "false"))
        .and(header("origin", "https://music.youtube.com"))
        .and(body_partial_json(browse_body))
        .respond_with(json_body(response))
        .expect(1)
        .mount(server)
        .await;
}

#[tokio::test]
async fn the_anonymous_home_sends_the_web_client_context_without_cookies() {
    let server = MockServer::start().await;
    let context = json!({"context": {"client": {"clientName": "WEB_REMIX", "hl": "pt", "gl": "BR"}}, "browseId": "FEmusic_home"});
    mount_browse(&server, context, HOME).await;
    let shelves = client(&server).home().await.unwrap();
    assert_eq!(shelves.len(), 2);
    let requests = server.received_requests().await.unwrap();
    assert!(requests[0].headers.get("cookie").is_none());
    assert!(requests[0].headers.get("authorization").is_none());
}

#[tokio::test]
async fn signed_in_requests_carry_cookies_and_a_sapisid_hash() {
    let server = MockServer::start().await;
    mount_browse(&server, json!({"browseId": "FEmusic_home"}), HOME).await;
    let auth = MusicAuth::from_netscape_cookies(COOKIES, 0);
    let signed_in = client(&server).with_auth(auth);
    assert!(signed_in.is_signed_in());
    signed_in.home().await.unwrap();
    let requests = server.received_requests().await.unwrap();
    let headers = &requests[0].headers;
    let value = |name: &str| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned()
    };
    assert_eq!(value("cookie"), "SAPISID=fixture-sapisid; SID=fixture-sid");
    assert!(value("authorization").starts_with("SAPISIDHASH "));
    assert_eq!(value("x-origin"), "https://music.youtube.com");
    assert_eq!(value("x-goog-authuser"), "0");
}

#[tokio::test]
async fn charts_select_the_country_through_form_data() {
    let server = MockServer::start().await;
    let body = json!({"browseId": "FEmusic_charts", "formData": {"selectedValues": ["US"]}});
    mount_browse(&server, body, CHARTS).await;
    let shelves = client(&server).charts("US").await.unwrap();
    assert_eq!(shelves.len(), 1);
}

#[tokio::test]
async fn moods_come_from_explore_and_open_with_their_params() {
    let server = MockServer::start().await;
    mount_browse(&server, json!({"browseId": "FEmusic_explore"}), EXPLORE).await;
    let body = json!({"browseId": "FEmusic_moods_and_genres_category", "params": "ggMPOg1uXzZQbDB5eThLRTQ3"});
    mount_browse(&server, body, MOOD).await;
    let music = client(&server);
    let moods = music.mood_categories().await.unwrap();
    let shelves = music.mood(&moods[0].params).await.unwrap();
    assert_eq!(shelves.len(), 2);
}

#[tokio::test]
async fn collections_are_browsed_by_id_and_bad_ids_are_refused() {
    let server = MockServer::start().await;
    mount_browse(&server, json!({"browseId": "MPREb_g3H4u5nW8xD"}), ALBUM).await;
    let music = client(&server);
    let album = music.collection("MPREb_g3H4u5nW8xD").await.unwrap();
    assert_eq!(album.songs.len(), 3);
    assert_eq!(
        music.collection("../evil").await,
        Err(MusicError::InvalidId("../evil".into()))
    );
}

#[tokio::test]
async fn song_search_uses_the_songs_filter() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/search"))
        .and(body_partial_json(
            json!({"query": "lucky static", "params": "EgWKAQIIAWoMEA4QChADEAQQCRAF"}),
        ))
        .respond_with(json_body(SEARCH))
        .expect(1)
        .mount(&server)
        .await;
    let songs = client(&server)
        .search_songs("  lucky static ")
        .await
        .unwrap();
    assert_eq!(songs.len(), 4);
}

#[tokio::test]
async fn song_radio_asks_the_watch_endpoint_for_the_radio_playlist() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/next"))
        .and(body_partial_json(
            json!({"videoId": "invented001", "playlistId": "RDAMVMinvented001"}),
        ))
        .respond_with(json_body(NEXT))
        .expect(1)
        .mount(&server)
        .await;
    let music = client(&server);
    let radio = music.radio("invented001").await.unwrap();
    assert_eq!(radio.len(), 5);
    assert_eq!(
        music.radio("../evil").await,
        Err(MusicError::InvalidId("../evil".into()))
    );
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].headers.get("cookie").is_none());
}

#[tokio::test]
async fn a_watch_page_without_a_queue_is_a_decode_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/next"))
        .respond_with(json_body("{\"contents\": {}}"))
        .mount(&server)
        .await;
    assert!(matches!(
        client(&server).radio("invented001").await,
        Err(MusicError::Decode(_))
    ));
}

#[tokio::test]
async fn failures_map_to_status_and_decode_errors() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"browseId": "FEmusic_home"})))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(body_partial_json(
            json!({"browseId": "FEmusic_new_releases_albums"}),
        ))
        .respond_with(json_body("not json"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"browseId": "FEmusic_charts"})))
        .respond_with(json_body("{\"contents\": {}}"))
        .mount(&server)
        .await;
    let music = client(&server);
    let rejected = music.home().await.unwrap_err();
    assert_eq!(rejected, MusicError::Status(403));
    assert!(rejected.is_rejected_sign_in());
    assert!(matches!(
        music.new_releases().await,
        Err(MusicError::Decode(_))
    ));
    assert!(matches!(
        music.charts("BR").await,
        Err(MusicError::Decode(_))
    ));
}

#[tokio::test]
#[ignore = "needs network and FEISHIN_REAL_YTM=1"]
async fn real_youtube_music_answers_every_discover_request() {
    if std::env::var_os("FEISHIN_REAL_YTM").is_none() {
        return;
    }
    let music = MusicClient::new(locale()).unwrap();
    let home = music.home().await.unwrap();
    assert!(home.iter().any(|shelf| !shelf.items.is_empty()));
    let releases = music.new_releases().await.unwrap();
    let Some(ShelfItem::Collection(album)) = releases.first().and_then(|shelf| shelf.items.first())
    else {
        panic!("no new releases");
    };
    assert!(
        !music
            .collection(&album.browse_id)
            .await
            .unwrap()
            .songs
            .is_empty()
    );
    let charts = music.charts("US").await.unwrap();
    assert!(!charts.is_empty());
    let moods = music.mood_categories().await.unwrap();
    assert!(!music.mood(&moods[0].params).await.unwrap().is_empty());
    let found = music.search_songs("classical piano").await.unwrap();
    let Some(first) = found.first() else {
        panic!("no search results");
    };
    assert!(!music.radio(&first.video_id).await.unwrap().is_empty());
}
