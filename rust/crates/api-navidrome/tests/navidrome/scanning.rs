use feishin_api::MusicServer;
use feishin_core::model::LibraryScan;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer};

use crate::support::{server, subsonic_ok};

#[tokio::test]
async fn starting_a_scan_reports_it_running() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/startScan.view"))
        .respond_with(subsonic_ok(
            json!({ "scanStatus": { "scanning": true, "count": 12 } }),
        ))
        .expect(1)
        .mount(&mock)
        .await;

    let scan = server(&mock, Some("jwt-1"))
        .start_library_scan()
        .await
        .expect("scan started");

    assert_eq!(
        scan,
        LibraryScan {
            scanning: true,
            scanned_items: Some(12)
        }
    );
}

#[tokio::test]
async fn scan_status_reports_a_finished_scan() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/getScanStatus.view"))
        .respond_with(subsonic_ok(
            json!({ "scanStatus": { "scanning": false, "count": 4321 } }),
        ))
        .expect(1)
        .mount(&mock)
        .await;

    let scan = server(&mock, Some("jwt-1"))
        .library_scan()
        .await
        .expect("scan status");

    assert_eq!(
        scan,
        LibraryScan {
            scanning: false,
            scanned_items: Some(4321)
        }
    );
}
