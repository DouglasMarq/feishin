use feishin_api::MusicServer;
use feishin_core::model::Paging;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{
    AudioFormat, mount_ping, native_playlist_json, native_song_json, server, subsonic_entry,
    subsonic_ok,
};

fn native_track(entry_id: &str, media_file_id: &str) -> serde_json::Value {
    let mut track = native_song_json(entry_id, "Track");
    track["mediaFileId"] = json!(media_file_id);
    track["playlistId"] = json!("pl1");
    track
}

async fn mount_native_track_page(mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/playlist/pl1/tracks"))
        .and(query_param("_start", "100"))
        .and(query_param("_end", "200"))
        .and(query_param("_sort", "id"))
        .and(query_param("_order", "ASC"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-total-count", "250")
                .set_body_json(json!([
                    native_track("101", "s101"),
                    native_track("102", "s102")
                ])),
        )
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/playlist/pl1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(native_playlist_json("pl1", "Mix")))
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn native_playlist_pages_use_start_end_and_total_count() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.58.0", &[]).await;
    mount_native_track_page(&mock).await;

    let server = server(&mock, Some("jwt-1"));
    let paging = Paging {
        start_index: 100,
        limit: 100,
    };
    let page = server
        .playlist_song_page("pl1", paging)
        .await
        .expect("page");
    assert_eq!(page.total_count, Some(250));
    assert_eq!(page.start_index, 100);
    let ids: Vec<&str> = page.items.iter().map(|song| song.id.as_str()).collect();
    assert_eq!(ids, ["s101", "s102"]);

    let playlist = server.playlist("pl1").await.expect("playlist");
    assert_eq!(playlist.name, "Mix");
    assert_eq!(playlist.song_count, 2);
    assert_eq!(playlist.duration_ms, 600_000);
    assert_eq!(playlist.owner.as_deref(), Some("alice"));
}

async fn mount_subsonic_playlist(mock: &MockServer) {
    let entries: Vec<serde_json::Value> = (0..5)
        .map(|index| subsonic_entry(&format!("s{index}"), "Song", "al1", AudioFormat::Mp3))
        .collect();
    Mock::given(method("GET"))
        .and(path("/rest/getPlaylist.view"))
        .and(query_param("id", "pl1"))
        .respond_with(subsonic_ok(json!({
            "playlist": {
                "id": "pl1", "name": "Mix", "owner": "alice", "songCount": 5,
                "duration": 900, "entry": entries
            }
        })))
        .expect(2)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn without_a_session_playlists_page_locally_from_subsonic() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.58.0", &[]).await;
    mount_subsonic_playlist(&mock).await;

    let server = server(&mock, None);
    let paging = Paging {
        start_index: 3,
        limit: 2,
    };
    let page = server
        .playlist_song_page("pl1", paging)
        .await
        .expect("page");
    assert_eq!(page.total_count, Some(5));
    let ids: Vec<&str> = page.items.iter().map(|song| song.id.as_str()).collect();
    assert_eq!(ids, ["s3", "s4"]);

    let playlist = server.playlist("pl1").await.expect("playlist");
    assert_eq!(playlist.song_count, 5);
    assert_eq!(playlist.duration_ms, 900_000);
}
