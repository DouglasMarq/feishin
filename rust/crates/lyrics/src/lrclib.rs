use std::time::Duration;

use reqwest::header::USER_AGENT;
use serde::{Deserialize, Serialize};

use crate::fuzzy::{SearchItem, order_search_results};
use crate::{
    Lyrics, LyricsQuery, LyricsSource, MATCH_THRESHOLD, Result, lyrics_from_text, send_json,
};

const AGENT: &str = "LRCGET v0.2.0 (https://github.com/jeffvli/feishin)";
const TIMEOUT: Duration = Duration::from_secs(5);
/// Same tolerance lrclib applies server side on `/api/get`.
const DURATION_TOLERANCE_SECS: f64 = 2.0;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Track {
    #[serde(default)]
    name: String,
    #[serde(default)]
    artist_name: String,
    duration: Option<f64>,
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
}

#[derive(Serialize)]
struct GetParams<'a> {
    track_name: &'a str,
    artist_name: &'a str,
    album_name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration: Option<String>,
}

pub(crate) async fn fetch(
    client: &reqwest::Client,
    base: &str,
    query: &LyricsQuery,
) -> Result<Option<Lyrics>> {
    if query.title.is_empty() && query.artist.is_empty() {
        return Ok(None);
    }
    let duration = query.duration_ms.map(|ms| ms as f64 / 1000.0);
    let mut fallback = None;

    if !query.title.is_empty() {
        let album = query
            .album
            .as_deref()
            .filter(|album| !album.is_empty())
            .unwrap_or(&query.title);
        let request = client
            .get(format!("{base}/api/get"))
            .header(USER_AGENT, AGENT)
            .timeout(TIMEOUT)
            .query(&GetParams {
                track_name: &query.title,
                artist_name: &query.artist,
                album_name: album,
                duration: duration.map(|secs| secs.to_string()),
            });
        if let Some(lyrics) = send_json::<Track>(request)
            .await?
            .and_then(|track| track_lyrics(&track))
        {
            if lyrics.synced {
                return Ok(Some(lyrics));
            }
            fallback = Some(lyrics);
        }
    }

    let request = client
        .get(format!("{base}/api/search"))
        .header(USER_AGENT, AGENT)
        .query(&[("q", format!("{} {}", query.title, query.artist))]);
    let tracks: Vec<Track> = send_json(request).await?.unwrap_or_default();
    let items: Vec<SearchItem<'_>> = tracks
        .iter()
        .map(|track| SearchItem {
            name: &track.name,
            artist: &track.artist_name,
            is_sync: Some(
                track
                    .synced_lyrics
                    .as_deref()
                    .is_some_and(|s| !s.is_empty()),
            ),
        })
        .collect();

    for ranked in order_search_results(&query.title, &query.artist, &items) {
        let track = &tracks[ranked.index];
        if ranked.score > MATCH_THRESHOLD || !duration_matches(track.duration, duration) {
            continue;
        }
        let Some(lyrics) = track_lyrics(track) else {
            continue;
        };
        if lyrics.synced {
            return Ok(Some(lyrics));
        }
        fallback.get_or_insert(lyrics);
    }
    Ok(fallback)
}

fn duration_matches(candidate: Option<f64>, wanted: Option<f64>) -> bool {
    match (candidate, wanted) {
        (Some(candidate), Some(wanted)) => (candidate - wanted).abs() <= DURATION_TOLERANCE_SECS,
        _ => true,
    }
}

fn track_lyrics(track: &Track) -> Option<Lyrics> {
    [&track.synced_lyrics, &track.plain_lyrics]
        .into_iter()
        .flatten()
        .filter(|text| !text.trim().is_empty())
        .find_map(|text| lyrics_from_text(LyricsSource::Lrclib, text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn query() -> LyricsQuery {
        LyricsQuery {
            title: "Bohemian Rhapsody".to_owned(),
            artist: "Queen".to_owned(),
            album: None,
            duration_ms: Some(354_000),
        }
    }

    fn track(id: u64, name: &str, duration: f64, plain: Value, synced: Value) -> Value {
        json!({
            "id": id,
            "name": name,
            "trackName": name,
            "artistName": "Queen",
            "albumName": "A Night at the Opera",
            "duration": duration,
            "instrumental": false,
            "plainLyrics": plain,
            "syncedLyrics": synced
        })
    }

    async fn mount_get(server: &MockServer, response: ResponseTemplate) {
        Mock::given(method("GET"))
            .and(path("/api/get"))
            .respond_with(response)
            .mount(server)
            .await;
    }

    async fn mount_search(server: &MockServer, body: Value) {
        Mock::given(method("GET"))
            .and(path("/api/search"))
            .and(query_param("q", "Bohemian Rhapsody Queen"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(server)
            .await;
    }

    fn not_found() -> ResponseTemplate {
        ResponseTemplate::new(404).set_body_json(json!({
            "code": 404, "name": "TrackNotFound", "message": "Failed to find specified track"
        }))
    }

    #[tokio::test]
    async fn get_endpoint_sends_track_signature() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/get"))
            .and(query_param("track_name", "Bohemian Rhapsody"))
            .and(query_param("artist_name", "Queen"))
            .and(query_param("album_name", "Bohemian Rhapsody"))
            .and(query_param("duration", "354"))
            .and(header("user-agent", AGENT))
            .respond_with(ResponseTemplate::new(200).set_body_json(track(
                1,
                "Bohemian Rhapsody",
                354.0,
                json!("Is this the real life?"),
                json!("[00:00.50] Is this the real life?\n[00:04.10] Is this just fantasy?"),
            )))
            .expect(1)
            .mount(&server)
            .await;

        let lyrics = fetch(&reqwest::Client::new(), &server.uri(), &query())
            .await
            .unwrap()
            .unwrap();
        assert!(lyrics.synced);
        assert_eq!(lyrics.source, LyricsSource::Lrclib);
        assert_eq!(lyrics.lines.len(), 2);
        assert_eq!(lyrics.lines[1].time_ms, Some(4100));
        assert_eq!(lyrics.lines[1].text, "Is this just fantasy?");
    }

    #[tokio::test]
    async fn plain_get_result_prefers_synced_search_hit() {
        let server = MockServer::start().await;
        mount_get(
            &server,
            ResponseTemplate::new(200).set_body_json(track(
                1,
                "Bohemian Rhapsody",
                354.0,
                json!("Plain only"),
                Value::Null,
            )),
        )
        .await;
        mount_search(
            &server,
            json!([
                track(2, "Bohemian Rhapsody", 354.5, json!("Plain"), Value::Null),
                track(
                    3,
                    "Bohemian Rhapsody",
                    355.0,
                    json!("Synced plain"),
                    json!("[00:01.00] Synced")
                )
            ]),
        )
        .await;

        let lyrics = fetch(&reqwest::Client::new(), &server.uri(), &query())
            .await
            .unwrap()
            .unwrap();
        assert!(lyrics.synced);
        assert_eq!(lyrics.lines[0].text, "Synced");
    }

    #[tokio::test]
    async fn keeps_plain_get_result_when_search_has_nothing_synced() {
        let server = MockServer::start().await;
        mount_get(
            &server,
            ResponseTemplate::new(200).set_body_json(track(
                1,
                "Bohemian Rhapsody",
                354.0,
                json!("Line one\nLine two"),
                Value::Null,
            )),
        )
        .await;
        mount_search(&server, json!([])).await;

        let lyrics = fetch(&reqwest::Client::new(), &server.uri(), &query())
            .await
            .unwrap()
            .unwrap();
        assert!(!lyrics.synced);
        assert_eq!(lyrics.lines.len(), 2);
        assert_eq!(lyrics.lines[0].time_ms, None);
    }

    #[tokio::test]
    async fn search_skips_wrong_duration_and_poor_matches() {
        let server = MockServer::start().await;
        mount_get(&server, not_found()).await;
        mount_search(
            &server,
            json!([
                track(
                    2,
                    "Bohemian Rhapsody",
                    300.0,
                    Value::Null,
                    json!("[00:01.00] wrong length")
                ),
                track(
                    3,
                    "Totally Unrelated Tune",
                    354.0,
                    Value::Null,
                    json!("[00:01.00] unrelated")
                ),
                track(
                    4,
                    "Bohemian Rhapsody (Remastered 2011)",
                    355.5,
                    json!("right"),
                    json!("[00:02.00] right")
                )
            ]),
        )
        .await;

        let lyrics = fetch(&reqwest::Client::new(), &server.uri(), &query())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(lyrics.lines[0].text, "right");
        assert_eq!(lyrics.lines[0].time_ms, Some(2000));
    }

    #[tokio::test]
    async fn instrumental_and_missing_tracks_yield_none() {
        let server = MockServer::start().await;
        mount_get(&server, not_found()).await;
        mount_search(
            &server,
            json!([track(
                2,
                "Bohemian Rhapsody",
                354.0,
                Value::Null,
                Value::Null
            )]),
        )
        .await;

        let lyrics = fetch(&reqwest::Client::new(), &server.uri(), &query())
            .await
            .unwrap();
        assert!(lyrics.is_none());
    }

    #[tokio::test]
    async fn malformed_json_is_a_decode_error() {
        let server = MockServer::start().await;
        mount_get(
            &server,
            ResponseTemplate::new(200).set_body_string("<html>not json</html>"),
        )
        .await;
        let result = fetch(&reqwest::Client::new(), &server.uri(), &query()).await;
        assert!(matches!(result, Err(crate::LyricsError::Decode(_))));
    }
}
