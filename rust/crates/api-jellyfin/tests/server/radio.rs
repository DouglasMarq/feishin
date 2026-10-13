use feishin_api::{ApiError, MusicServer};
use feishin_core::model::RadioStationDraft;
use wiremock::MockServer;

use crate::support::server;

fn draft() -> RadioStationDraft {
    RadioStationDraft {
        name: "Jazz FM".into(),
        stream_url: "https://stream.example.com/jazz".into(),
        homepage_url: None,
    }
}

#[tokio::test]
async fn jellyfin_has_no_server_stations_and_makes_no_requests() {
    let mock = MockServer::start().await;
    let server = server(&mock);

    assert!(server.radio_stations().await.unwrap().is_empty());
    assert!(!server.can_manage_radio_stations().await.unwrap());
    assert!(
        mock.received_requests()
            .await
            .unwrap_or_default()
            .is_empty()
    );
}

#[tokio::test]
async fn station_changes_are_unsupported() {
    let mock = MockServer::start().await;
    let server = server(&mock);

    assert!(matches!(
        server.create_radio_station(&draft()).await,
        Err(ApiError::Unsupported(_))
    ));
    assert!(matches!(
        server.update_radio_station("1", &draft()).await,
        Err(ApiError::Unsupported(_))
    ));
    assert!(matches!(
        server.delete_radio_station("1").await,
        Err(ApiError::Unsupported(_))
    ));
}
