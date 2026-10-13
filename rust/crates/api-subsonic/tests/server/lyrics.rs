use feishin_api::MusicServer;
use serde_json::json;
use wiremock::matchers::{path, query_param};
use wiremock::{Mock, MockServer};

use crate::support::{mount_extensions, mount_plain_ping, ok, server, server_song};

#[tokio::test]
async fn lyrics_use_song_lyrics_extension() {
    let mock = MockServer::start().await;
    mount_extensions(&mock, json!([{ "name": "songLyrics", "versions": [1, 2] }])).await;
    Mock::given(path("/rest/getLyricsBySongId.view"))
        .and(query_param("id", "s1"))
        .and(query_param("enhanced", "true"))
        .respond_with(ok(json!({
            "lyricsList": { "structuredLyrics": [
                { "displayArtist": "Artist", "displayTitle": "One", "lang": "eng", "synced": true,
                  "offset": 100, "line": [ { "start": 0, "value": "Hello" }, { "start": 2500, "value": "World" } ] }
            ] }
        })))
        .expect(1)
        .mount(&mock)
        .await;
    let server = server(&mock);
    let song = server_song(&server);

    let lyrics = server.lyrics(&song).await.expect("lyrics");
    assert_eq!(
        lyrics.as_deref(),
        Some("[offset:100]\n[00:00.00]Hello\n[00:02.50]World")
    );
}

#[tokio::test]
async fn lyrics_without_extension_skip_request() {
    let mock = MockServer::start().await;
    mount_plain_ping(&mock).await;
    Mock::given(path("/rest/getLyricsBySongId.view"))
        .respond_with(ok(json!({})))
        .expect(0)
        .mount(&mock)
        .await;
    let server = server(&mock);
    let song = server_song(&server);
    assert_eq!(server.lyrics(&song).await.ok(), Some(None));
}
