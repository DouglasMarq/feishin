use feishin_api::{ApiError, MusicServer};
use feishin_core::model::{RadioStation, RadioStationDraft};
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer};

use crate::support::{server, subsonic_failed, subsonic_ok};

fn draft() -> RadioStationDraft {
    RadioStationDraft {
        name: "Jazz FM".into(),
        stream_url: "https://stream.example.com/jazz".into(),
        homepage_url: Some("https://jazz.example.com".into()),
    }
}

#[tokio::test]
async fn lists_internet_radio_stations() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/getInternetRadioStations.view"))
        .respond_with(subsonic_ok(json!({
            "internetRadioStations": { "internetRadioStation": [
                { "id": "abc", "name": "Jazz FM", "streamUrl": "https://stream.example.com/jazz",
                  "homePageUrl": "https://jazz.example.com" },
                { "id": "def", "name": "News", "streamUrl": "https://stream.example.com/news" }
            ] }
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let stations = server(&mock, Some("jwt"))
        .radio_stations()
        .await
        .expect("stations");

    assert_eq!(
        stations,
        vec![
            RadioStation {
                id: "abc".into(),
                name: "Jazz FM".into(),
                stream_url: "https://stream.example.com/jazz".into(),
                homepage_url: Some("https://jazz.example.com".into()),
            },
            RadioStation {
                id: "def".into(),
                name: "News".into(),
                stream_url: "https://stream.example.com/news".into(),
                homepage_url: None,
            },
        ]
    );
}

#[tokio::test]
async fn management_follows_the_admin_role() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/getUser.view"))
        .and(query_param("username", "alice"))
        .respond_with(subsonic_ok(
            json!({ "user": { "username": "alice", "adminRole": true } }),
        ))
        .expect(1)
        .mount(&mock)
        .await;

    let allowed = server(&mock, Some("jwt"))
        .can_manage_radio_stations()
        .await
        .expect("user");

    assert!(allowed);
}

#[tokio::test]
async fn creates_updates_and_deletes_stations() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/createInternetRadioStation.view"))
        .and(query_param("name", "Jazz FM"))
        .and(query_param("streamUrl", "https://stream.example.com/jazz"))
        .and(query_param("homepageUrl", "https://jazz.example.com"))
        .respond_with(subsonic_ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/updateInternetRadioStation.view"))
        .and(query_param("id", "abc"))
        .and(query_param("name", "Jazz FM"))
        .respond_with(subsonic_ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/deleteInternetRadioStation.view"))
        .and(query_param("id", "abc"))
        .respond_with(subsonic_ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    let server = server(&mock, Some("jwt"));

    server.create_radio_station(&draft()).await.expect("create");
    server
        .update_radio_station("abc", &draft())
        .await
        .expect("update");
    server.delete_radio_station("abc").await.expect("delete");
}

#[tokio::test]
async fn refused_changes_are_errors() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/deleteInternetRadioStation.view"))
        .respond_with(subsonic_failed(50, "user not authorized"))
        .mount(&mock)
        .await;

    let refused = server(&mock, Some("jwt")).delete_radio_station("abc").await;

    assert!(matches!(refused, Err(ApiError::Server { .. })));
}
