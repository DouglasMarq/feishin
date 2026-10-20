use super::*;
use serde_json::{Value, json};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn query() -> LyricsQuery {
    LyricsQuery {
        title: "Lime Harbor".to_owned(),
        artist: "Mizuho Lantern".to_owned(),
        album: None,
        duration_ms: Some(255_000),
    }
}

fn song(id: u64, name: &str, artists: &[&str]) -> Value {
    json!({
        "id": id,
        "name": name,
        "artists": artists.iter().enumerate().map(|(artist_index, artist_name)| json!({
            "id": artist_index, "name": artist_name, "picUrl": null, "alias": [], "albumSize": 0,
            "picId": 0, "fansGroup": null, "img1v1Url": "", "img1v1": 0, "trans": null
        })).collect::<Vec<_>>(),
        "album": { "id": 9, "name": "Lime Harbor", "artist": { "id": 0, "name": "" },
                   "publishTime": 0, "size": 1, "copyrightId": 0, "status": 0, "picId": 0, "mark": 0 },
        "duration": 255_000,
        "copyrightId": 0, "status": 0, "alias": [], "rtype": 0, "ftype": 0,
        "mvid": 0, "fee": 0, "rUrl": null, "mark": 0
    })
}

async fn mount_search(server: &MockServer, songs: Vec<Value>) {
    Mock::given(method("GET"))
        .and(path("/api/search/get"))
        .and(query_param("s", "Mizuho Lantern Lime Harbor"))
        .and(query_param("type", "1"))
        .and(query_param("limit", "5"))
        .and(query_param("offset", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": { "songs": songs, "hasMore": false, "songCount": 2 },
            "code": 200
        })))
        .mount(server)
        .await;
}

async fn mount_lyric(server: &MockServer, id: &str, lyric: &str) {
    Mock::given(method("GET"))
        .and(path("/api/song/lyric"))
        .and(query_param("id", id))
        .and(query_param("kv", "-1"))
        .and(query_param("lv", "-1"))
        .and(query_param("tv", "-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "sgc": false, "sfy": false, "qfy": false,
            "lrc": { "version": 3, "lyric": lyric },
            "klyric": { "version": 0, "lyric": "" },
            "tlyric": { "version": 0, "lyric": "" },
            "code": 200
        })))
        .mount(server)
        .await;
}

#[tokio::test]
async fn fetches_synced_lyrics_for_best_match() {
    let server = MockServer::start().await;
    mount_search(
        &server,
        vec![
            song(111, "Lime Harbor", &["Mizuho Lantern"]),
            song(222, "Something Else", &["Other Person"]),
        ],
    )
    .await;
    mount_lyric(
        &server,
        "111",
        "[by:someone]\n[00:00.00] 作词 : 森川青\n[00:12.345]星がきらめく夜の港で\n",
    )
    .await;

    let lyrics = fetch(&reqwest::Client::new(), &server.uri(), &query())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(lyrics.source, LyricsSource::NetEase);
    assert!(lyrics.is_synced);
    assert_eq!(lyrics.lines.len(), 2);
    assert_eq!(lyrics.lines[1].time_ms, Some(12_345));
    assert_eq!(lyrics.lines[1].text, "星がきらめく夜の港で");
}

#[tokio::test]
async fn prefers_a_synced_candidate_over_plain_one() {
    let server = MockServer::start().await;
    mount_search(
        &server,
        vec![
            song(111, "Lime Harbor", &["Mizuho Lantern"]),
            song(333, "Lime Harbor (Live)", &["Mizuho Lantern", "Guest"]),
        ],
    )
    .await;
    mount_lyric(&server, "111", "plain words only").await;
    mount_lyric(&server, "333", "[00:01.00]synced").await;

    let lyrics = fetch(&reqwest::Client::new(), &server.uri(), &query())
        .await
        .unwrap()
        .unwrap();
    assert!(lyrics.is_synced);
    assert_eq!(lyrics.lines[0].text, "synced");
}

#[tokio::test]
async fn no_match_when_results_are_unrelated() {
    let server = MockServer::start().await;
    mount_search(&server, vec![song(222, "Zzzzzz", &["Qqqqqq"])]).await;
    let lyrics = fetch(&reqwest::Client::new(), &server.uri(), &query())
        .await
        .unwrap();
    assert!(lyrics.is_none());
}

#[tokio::test]
async fn missing_result_or_lyric_yields_none() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/search/get"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "result": {}, "code": 200 })),
        )
        .mount(&server)
        .await;
    let lyrics = fetch(&reqwest::Client::new(), &server.uri(), &query())
        .await
        .unwrap();
    assert!(lyrics.is_none());

    let server = MockServer::start().await;
    mount_search(&server, vec![song(111, "Lime Harbor", &["Mizuho Lantern"])]).await;
    Mock::given(method("GET"))
        .and(path("/api/song/lyric"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({ "sgc": true, "nolyric": true, "uncollected": true, "code": 200 }),
        ))
        .mount(&server)
        .await;
    let lyrics = fetch(&reqwest::Client::new(), &server.uri(), &query())
        .await
        .unwrap();
    assert!(lyrics.is_none());
}
