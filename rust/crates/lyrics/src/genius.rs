use scraper::{ElementRef, Html, Selector};
use serde::Deserialize;

use crate::fuzzy::{SearchItem, order_search_results};
use crate::{
    Lyrics, LyricsQuery, LyricsSource, MATCH_THRESHOLD, Result, lyrics_from_text, send_json,
    send_text,
};

#[derive(Debug, Deserialize)]
struct SearchResponse {
    response: Option<ResponseBody>,
}

#[derive(Debug, Deserialize)]
struct ResponseBody {
    sections: Option<Vec<Section>>,
}

#[derive(Debug, Deserialize)]
struct Section {
    hits: Option<Vec<Hit>>,
}

#[derive(Debug, Deserialize)]
struct Hit {
    result: Option<Song>,
}

#[derive(Debug, Deserialize)]
struct Song {
    #[serde(default)]
    artist_names: String,
    #[serde(default)]
    full_title: String,
    url: String,
}

pub(crate) async fn fetch(
    client: &reqwest::Client,
    base: &str,
    query: &LyricsQuery,
) -> Result<Option<Lyrics>> {
    if query.title.is_empty() && query.artist.is_empty() {
        return Ok(None);
    }
    let search_query = format!("{} {}", query.artist, query.title);
    let request = client
        .get(format!("{base}/api/search/song"))
        .query(&[("per_page", "5"), ("q", search_query.as_str())]);
    let songs: Vec<Song> = send_json::<SearchResponse>(request)
        .await?
        .and_then(|response| response.response)
        .and_then(|body| body.sections)
        .and_then(|sections| sections.into_iter().next())
        .and_then(|section| section.hits)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|hit| hit.result)
        .collect();

    let items: Vec<SearchItem<'_>> = songs
        .iter()
        .map(|song| SearchItem {
            name: &song.full_title,
            artist: &song.artist_names,
            is_sync: None,
        })
        .collect();

    for ranked in order_search_results(&query.title, &query.artist, &items) {
        if ranked.score > MATCH_THRESHOLD {
            continue;
        }
        let Some(html) = send_text(client.get(&songs[ranked.index].url)).await? else {
            continue;
        };
        if let Some(lyrics) = lyrics_from_text(LyricsSource::Genius, &extract_lyrics(&html)) {
            return Ok(Some(lyrics));
        }
    }
    Ok(None)
}

fn extract_lyrics(html: &str) -> String {
    let document = Html::parse_document(html);
    let (Ok(legacy), Ok(containers)) = (
        Selector::parse("div.lyrics"),
        Selector::parse(r#"div[data-lyrics-container="true"]"#),
    ) else {
        return String::new();
    };

    let legacy: Vec<ElementRef<'_>> = document.select(&legacy).collect();
    if !legacy.is_empty() {
        let mut text = String::new();
        for element in legacy {
            collect_text(element, &mut text);
        }
        return text.trim().to_owned();
    }

    document
        .select(&containers)
        .map(|element| {
            let mut text = String::new();
            collect_text(element, &mut text);
            text
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn collect_text(element: ElementRef<'_>, out: &mut String) {
    for child in element.children() {
        if let Some(text) = child.value().as_text() {
            out.push_str(text);
        } else if let Some(child) = ElementRef::wrap(child) {
            let value = child.value();
            if value.name() == "br" {
                out.push('\n');
            } else if value.attr("data-exclude-from-selection") != Some("true") {
                collect_text(child, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const PAGE: &str = r#"<!DOCTYPE html><html><head><title>Queen - Bohemian Rhapsody Lyrics | Genius Lyrics</title></head>
<body><main>
<div class="Lyrics__Container" data-lyrics-container="true"><div data-exclude-from-selection="true"><span>32 Contributors</span>Bohemian Rhapsody Lyrics</div>[Intro]<br/>Is this the real life?<br>Is this just <i>fantasy</i>?</div>
<div class="LyricsFooter">Embed</div>
<div class="Lyrics__Container" data-lyrics-container="true">[Verse 1]<br/>Mama, just killed a man &amp; more</div>
</main></body></html>"#;

    fn query() -> LyricsQuery {
        LyricsQuery {
            title: "Bohemian Rhapsody".to_owned(),
            artist: "Queen".to_owned(),
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
            .and(query_param("q", "Queen Bohemian Rhapsody"))
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
            "[Intro]\nIs this the real life?\nIs this just fantasy?\n[Verse 1]\nMama, just killed a man & more"
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
                    "Queen-bohemian-rhapsody-lyrics",
                    "Bohemian Rhapsody",
                    "Queen",
                ),
                hit(
                    &server,
                    "Other-song-lyrics",
                    "Under Pressure",
                    "Queen & David Bowie",
                ),
            ],
        )
        .await;
        Mock::given(method("GET"))
            .and(path("/Queen-bohemian-rhapsody-lyrics"))
            .respond_with(ResponseTemplate::new(200).set_body_string(PAGE))
            .expect(1)
            .mount(&server)
            .await;

        let lyrics = fetch(&reqwest::Client::new(), &server.uri(), &query())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(lyrics.source, LyricsSource::Genius);
        assert!(!lyrics.synced);
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
                "Queen-bohemian-rhapsody-lyrics",
                "Bohemian Rhapsody",
                "Queen",
            )],
        )
        .await;
        Mock::given(method("GET"))
            .and(path("/Queen-bohemian-rhapsody-lyrics"))
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
}
