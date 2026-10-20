use super::*;
use serde_json::{Value, json};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PAGE: &str = r#"<!DOCTYPE html><html><head><title>Velvet Comets - Lantern Harbor Lyrics | Genius Lyrics</title></head>
<body><main>
<div class="Lyrics__Container" data-lyrics-container="true"><div data-exclude-from-selection="true"><span>32 Contributors</span>Lantern Harbor Lyrics</div>[Intro]<br/>Do the lanterns still glow?<br>Do they only <i>flicker</i>?</div>
<div class="LyricsFooter">Embed</div>
<div class="Lyrics__Container" data-lyrics-container="true">[Verse 1]<br/>Paper boats drift by &amp; more</div>
</main></body></html>"#;

fn query() -> LyricsQuery {
    LyricsQuery {
        title: "Lantern Harbor".to_owned(),
        artist: "Velvet Comets".to_owned(),
        album: None,
        duration_ms: None,
    }
}

fn hit(server: &MockServer, slug: &str, title: &str, artist: &str) -> Value {
    json!({
        "highlights": [],
        "index": "song",
        "type": "song",
        "result": {
            "_type": "song",
            "annotation_count": 1,
            "api_path": format!("/songs/{slug}"),
            "artist_names": artist,
            "full_title": format!("{title} by {artist}"),
            "id": 1,
            "instrumental": false,
            "lyrics_state": "complete",
            "path": format!("/{slug}"),
            "primary_artist": { "_type": "artist", "id": 2, "name": artist, "url": "https://genius.com/artists/x" },
            "title": title,
            "title_with_featured": title,
            "url": format!("{}/{slug}", server.uri())
        }
    })
}

async fn mount_search(server: &MockServer, hits: Vec<Value>) {
    Mock::given(method("GET"))
        .and(path("/api/search/song"))
        .and(query_param("per_page", "5"))
        .and(query_param("q", "Velvet Comets Lantern Harbor"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "meta": { "status": 200 },
            "response": { "sections": [{ "type": "song", "hits": hits }], "next_page": 2 }
        })))
        .mount(server)
        .await;
}

#[test]
fn extracts_modern_containers() {
    assert_eq!(
        extract_lyrics(PAGE),
        "[Intro]\nDo the lanterns still glow?\nDo they only flicker?\n[Verse 1]\nPaper boats drift by & more"
    );
}

#[test]
fn extracts_legacy_lyrics_div() {
    let html = r#"<html><body><div class="lyrics">
  <p>Line one<br/>Line two</p>
</div></body></html>"#;
    assert_eq!(extract_lyrics(html), "Line one\nLine two");
}

#[tokio::test]
async fn fetches_and_scrapes_best_hit() {
    let server = MockServer::start().await;
    mount_search(
        &server,
        vec![
            hit(
                &server,
                "Velvet-comets-lantern-harbor-lyrics",
                "Lantern Harbor",
                "Velvet Comets",
            ),
            hit(
                &server,
                "Other-song-lyrics",
                "Tidal Pressure",
                "Velvet Comets & Iris Vale",
            ),
        ],
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/Velvet-comets-lantern-harbor-lyrics"))
        .respond_with(ResponseTemplate::new(200).set_body_string(PAGE))
        .expect(1)
        .mount(&server)
        .await;

    let lyrics = fetch(&reqwest::Client::new(), &server.uri(), &query())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(lyrics.source, LyricsSource::Genius);
    assert!(!lyrics.is_synced);
    assert_eq!(lyrics.lines.len(), 5);
    assert_eq!(lyrics.lines[0].text, "[Intro]");
    assert!(lyrics.lines.iter().all(|line| line.time_ms.is_none()));
}

#[tokio::test]
async fn empty_sections_yield_none() {
    let server = MockServer::start().await;
    mount_search(&server, vec![]).await;
    let lyrics = fetch(&reqwest::Client::new(), &server.uri(), &query())
        .await
        .unwrap();
    assert!(lyrics.is_none());
}

#[tokio::test]
async fn page_without_lyrics_yields_none() {
    let server = MockServer::start().await;
    mount_search(
        &server,
        vec![hit(
            &server,
            "Velvet-comets-lantern-harbor-lyrics",
            "Lantern Harbor",
            "Velvet Comets",
        )],
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/Velvet-comets-lantern-harbor-lyrics"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string("<html><body>nothing</body></html>"),
        )
        .mount(&server)
        .await;
    let lyrics = fetch(&reqwest::Client::new(), &server.uri(), &query())
        .await
        .unwrap();
    assert!(lyrics.is_none());
}

#[tokio::test]
async fn given_a_hit_outside_genius_its_page_is_never_fetched() {
    let server = MockServer::start().await;
    let elsewhere = MockServer::start().await;
    let mut foreign = hit(
        &server,
        "Velvet-comets-lantern-harbor-lyrics",
        "Lantern Harbor",
        "Velvet Comets",
    );
    foreign["result"]["url"] = json!(format!(
        "{}/Velvet-comets-lantern-harbor-lyrics",
        elsewhere.uri()
    ));
    mount_search(&server, vec![foreign]).await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(PAGE))
        .expect(0)
        .mount(&elsewhere)
        .await;

    let lyrics = fetch(&reqwest::Client::new(), &server.uri(), &query())
        .await
        .unwrap();

    assert!(lyrics.is_none());
}

#[tokio::test]
async fn given_a_page_larger_than_the_cap_it_is_rejected() {
    let server = MockServer::start().await;
    mount_search(
        &server,
        vec![hit(
            &server,
            "Velvet-comets-lantern-harbor-lyrics",
            "Lantern Harbor",
            "Velvet Comets",
        )],
    )
    .await;
    let mut oversized = PAGE.to_owned();
    oversized.push_str(&" ".repeat(crate::http::MAX_RESPONSE_BYTES));
    Mock::given(method("GET"))
        .and(path("/Velvet-comets-lantern-harbor-lyrics"))
        .respond_with(ResponseTemplate::new(200).set_body_string(oversized))
        .mount(&server)
        .await;

    let fetched = fetch(&reqwest::Client::new(), &server.uri(), &query()).await;

    assert!(matches!(fetched, Err(crate::LyricsError::Decode(_))));
}
