use feishin_api::{ApiError, MusicServer};
use feishin_core::model::{RadioStation, RadioStationDraft};
use serde_json::json;
use wiremock::matchers::{path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer};

use crate::support::{failed, ok, server};

fn draft() -> RadioStationDraft {
    RadioStationDraft {
        name: "Jazz FM".into(),
        stream_url: "https://stream.example.com/jazz".into(),
        homepage_url: None,
    }
}

#[tokio::test]
async fn lists_internet_radio_stations() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/getInternetRadioStations.view"))
        .respond_with(ok(json!({
            "internetRadioStations": { "internetRadioStation": [
                { "id": 1, "name": "Jazz FM", "streamUrl": "https://stream.example.com/jazz",
                  "homePageUrl": "https://jazz.example.com" },
                { "id": "r2", "name": "Talk", "streamUrl": "https://stream.example.com/talk",
                  "homePageUrl": "" }
            ] }
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let stations = server(&mock).radio_stations().await.expect("stations");

    assert_eq!(
        stations,
        vec![
            RadioStation {
                id: "1".into(),
                name: "Jazz FM".into(),
                stream_url: "https://stream.example.com/jazz".into(),
                homepage_url: Some("https://jazz.example.com".into()),
            },
            RadioStation {
                id: "r2".into(),
                name: "Talk".into(),
                stream_url: "https://stream.example.com/talk".into(),
                homepage_url: None,
            },
        ]
    );
}

#[tokio::test]
async fn an_empty_station_list_is_empty() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/getInternetRadioStations.view"))
        .respond_with(ok(json!({ "internetRadioStations": {} })))
        .mount(&mock)
        .await;

    assert!(
        server(&mock)
            .radio_stations()
            .await
            .expect("stations")
            .is_empty()
    );
}

#[tokio::test]
async fn admins_may_manage_stations() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/getUser.view"))
        .and(query_param("username", "alice"))
        .respond_with(ok(
            json!({ "user": { "username": "alice", "adminRole": true } }),
        ))
        .expect(1)
        .mount(&mock)
        .await;

    assert!(
        server(&mock)
            .can_manage_radio_stations()
            .await
            .expect("user")
    );
}

#[tokio::test]
async fn regular_users_may_not_manage_stations() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/getUser.view"))
        .respond_with(ok(
            json!({ "user": { "username": "alice", "adminRole": false } }),
        ))
        .mount(&mock)
        .await;

    assert!(
        !server(&mock)
            .can_manage_radio_stations()
            .await
            .expect("user")
    );
}

#[tokio::test]
async fn creates_updates_and_deletes_stations() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/createInternetRadioStation.view"))
        .and(query_param("name", "Jazz FM"))
        .and(query_param("streamUrl", "https://stream.example.com/jazz"))
        .and(query_param_is_missing("homepageUrl"))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(path("/rest/updateInternetRadioStation.view"))
        .and(query_param("id", "7"))
        .and(query_param("homepageUrl", "https://jazz.example.com"))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(path("/rest/deleteInternetRadioStation.view"))
        .and(query_param("id", "7"))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    let server = server(&mock);

    server.create_radio_station(&draft()).await.expect("create");
    let with_homepage = RadioStationDraft {
        homepage_url: Some("https://jazz.example.com".into()),
        ..draft()
    };
    server
        .update_radio_station("7", &with_homepage)
        .await
        .expect("update");
    server.delete_radio_station("7").await.expect("delete");
}

#[tokio::test]
async fn refused_changes_are_errors() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/createInternetRadioStation.view"))
        .respond_with(failed(json!(50), "not authorized"))
        .mount(&mock)
        .await;

    let refused = server(&mock).create_radio_station(&draft()).await;

    assert!(matches!(refused, Err(ApiError::Server { code: 50, .. })));
}
