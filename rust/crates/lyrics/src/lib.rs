//! External lyrics providers (port of `src/main/features/core/lyrics`) and LRC parsing.

mod fuzzy;
mod genius;
mod lrc;
mod lrclib;
mod netease;

use serde::de::DeserializeOwned;

#[derive(Debug, thiserror::Error)]
pub enum LyricsError {
    #[error("network error: {0}")]
    Network(String),
    #[error("unexpected response: {0}")]
    Decode(String),
}

pub type Result<T> = std::result::Result<T, LyricsError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LyricsSource {
    Lrclib,
    NetEase,
    Genius,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricLine {
    /// `None` for unsynced lyrics.
    pub time_ms: Option<u64>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lyrics {
    pub source: LyricsSource,
    pub synced: bool,
    pub lines: Vec<LyricLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsQuery {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
}

/// Highest fuzzy score (0 best, 1 worst) accepted as a match, as in `getRemoteLyrics`.
const MATCH_THRESHOLD: f64 = 0.55;

#[derive(Debug, Clone)]
struct Endpoints {
    lrclib: String,
    netease: String,
    genius: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            lrclib: "https://lrclib.net".to_owned(),
            netease: "https://music.163.com".to_owned(),
            genius: "https://genius.com".to_owned(),
        }
    }
}

/// Tries `sources` in order and returns the first match, preferring synced lyrics within a source.
///
/// A failing source is skipped; an error is returned only when every source failed.
pub async fn fetch(
    client: &reqwest::Client,
    query: &LyricsQuery,
    sources: &[LyricsSource],
) -> Result<Option<Lyrics>> {
    fetch_from(client, query, sources, &Endpoints::default()).await
}

async fn fetch_from(
    client: &reqwest::Client,
    query: &LyricsQuery,
    sources: &[LyricsSource],
    endpoints: &Endpoints,
) -> Result<Option<Lyrics>> {
    let mut first_error = None;
    let mut any_ok = false;
    for &source in sources {
        let result = match source {
            LyricsSource::Lrclib => lrclib::fetch(client, &endpoints.lrclib, query).await,
            LyricsSource::NetEase => netease::fetch(client, &endpoints.netease, query).await,
            LyricsSource::Genius => genius::fetch(client, &endpoints.genius, query).await,
        };
        match result {
            Ok(Some(lyrics)) => return Ok(Some(lyrics)),
            Ok(None) => any_ok = true,
            Err(error) => {
                tracing::warn!(?source, %error, "lyrics source failed");
                first_error.get_or_insert(error);
            }
        }
    }
    match first_error {
        Some(error) if !any_ok => Err(error),
        _ => Ok(None),
    }
}

/// Parses LRC (`[mm:ss.xx] text`, multiple timestamps per line, offset tag). Plain text becomes unsynced lines.
pub fn parse_lrc(text: &str) -> Vec<LyricLine> {
    lrc::parse_lrc(text)
}

fn lyrics_from_text(source: LyricsSource, text: &str) -> Option<Lyrics> {
    let lines = parse_lrc(text);
    if lines.is_empty() {
        return None;
    }
    let synced = lines.iter().any(|line| line.time_ms.is_some());
    Some(Lyrics {
        source,
        synced,
        lines,
    })
}

async fn send(request: reqwest::RequestBuilder) -> Result<Option<reqwest::Response>> {
    let response = request
        .send()
        .await
        .map_err(|e| LyricsError::Network(e.to_string()))?;
    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !status.is_success() {
        return Err(LyricsError::Network(format!("HTTP {status}")));
    }
    Ok(Some(response))
}

async fn send_json<T: DeserializeOwned>(request: reqwest::RequestBuilder) -> Result<Option<T>> {
    let Some(response) = send(request).await? else {
        return Ok(None);
    };
    let body = response
        .bytes()
        .await
        .map_err(|e| LyricsError::Network(e.to_string()))?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|e| LyricsError::Decode(e.to_string()))
}

async fn send_text(request: reqwest::RequestBuilder) -> Result<Option<String>> {
    let Some(response) = send(request).await? else {
        return Ok(None);
    };
    response
        .text()
        .await
        .map(Some)
        .map_err(|e| LyricsError::Network(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn query() -> LyricsQuery {
        LyricsQuery {
            title: "Bohemian Rhapsody".to_owned(),
            artist: "Queen".to_owned(),
            album: Some("A Night at the Opera".to_owned()),
            duration_ms: Some(354_000),
        }
    }

    fn endpoints(server: &MockServer) -> Endpoints {
        Endpoints {
            lrclib: server.uri(),
            netease: server.uri(),
            genius: server.uri(),
        }
    }

    #[tokio::test]
    async fn returns_first_source_with_a_match() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/get"))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "code": 404, "name": "TrackNotFound", "message": "Failed to find specified track"
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/search/get"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "result": {
                    "songs": [{
                        "id": 1, "name": "Bohemian Rhapsody", "duration": 354_000,
                        "artists": [{ "id": 2, "name": "Queen" }],
                        "album": { "id": 3, "name": "A Night at the Opera" }
                    }],
                    "songCount": 1, "hasMore": false
                },
                "code": 200
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/song/lyric"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "lrc": { "version": 1, "lyric": "[00:01.00]Is this the real life?" },
                "code": 200
            })))
            .mount(&server)
            .await;

        let lyrics = fetch_from(
            &reqwest::Client::new(),
            &query(),
            &[
                LyricsSource::Lrclib,
                LyricsSource::NetEase,
                LyricsSource::Genius,
            ],
            &endpoints(&server),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(lyrics.source, LyricsSource::NetEase);
        assert!(lyrics.synced);
        assert_eq!(lyrics.lines[0].text, "Is this the real life?");
    }

    #[tokio::test]
    async fn errors_only_when_every_source_fails() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let endpoints = endpoints(&server);

        let all_failed = fetch_from(
            &client,
            &query(),
            &[LyricsSource::NetEase, LyricsSource::Genius],
            &endpoints,
        )
        .await;
        assert!(matches!(all_failed, Err(LyricsError::Network(_))));

        let none = fetch_from(&client, &query(), &[], &endpoints)
            .await
            .unwrap();
        assert!(none.is_none());
    }

    #[tokio::test]
    async fn failing_source_falls_through_to_next() {
        let failing = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&failing)
            .await;
        let working = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/get"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 7, "name": "Bohemian Rhapsody", "trackName": "Bohemian Rhapsody",
                "artistName": "Queen", "albumName": "A Night at the Opera", "duration": 354.0,
                "instrumental": false, "plainLyrics": "Is this the real life?",
                "syncedLyrics": "[00:01.00] Is this the real life?"
            })))
            .mount(&working)
            .await;

        let endpoints = Endpoints {
            lrclib: working.uri(),
            netease: failing.uri(),
            genius: failing.uri(),
        };
        let lyrics = fetch_from(
            &reqwest::Client::new(),
            &query(),
            &[LyricsSource::NetEase, LyricsSource::Lrclib],
            &endpoints,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(lyrics.source, LyricsSource::Lrclib);
        assert!(lyrics.synced);
    }
}
