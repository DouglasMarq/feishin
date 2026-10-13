use crate::{Lyrics, LyricsQuery, LyricsSource, Result, genius, lrclib, netease};

#[derive(Debug, Clone)]
pub(crate) struct Endpoints {
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

pub(crate) async fn fetch_from(
    client: &reqwest::Client,
    query: &LyricsQuery,
    sources: &[LyricsSource],
    endpoints: &Endpoints,
) -> Result<Option<Lyrics>> {
    let mut first_error = None;
    let mut any_source_answered = false;
    for &source in sources {
        match fetch_source(client, query, source, endpoints).await {
            Ok(Some(lyrics)) => return Ok(Some(lyrics)),
            Ok(None) => any_source_answered = true,
            Err(error) => {
                tracing::warn!(?source, %error, "lyrics source failed");
                first_error.get_or_insert(error);
            }
        }
    }
    match first_error {
        Some(error) if !any_source_answered => Err(error),
        _ => Ok(None),
    }
}

async fn fetch_source(
    client: &reqwest::Client,
    query: &LyricsQuery,
    source: LyricsSource,
    endpoints: &Endpoints,
) -> Result<Option<Lyrics>> {
    match source {
        LyricsSource::Lrclib => lrclib::fetch(client, &endpoints.lrclib, query).await,
        LyricsSource::NetEase => netease::fetch(client, &endpoints.netease, query).await,
        LyricsSource::Genius => genius::fetch(client, &endpoints.genius, query).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LyricsError;
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

    async fn mount_lrclib_miss(server: &MockServer) {
        Mock::given(method("GET"))
            .and(path("/api/get"))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "code": 404, "name": "TrackNotFound", "message": "Failed to find specified track"
            })))
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .mount(server)
            .await;
    }

    async fn mount_netease_match(server: &MockServer) {
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
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/song/lyric"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "lrc": { "version": 1, "lyric": "[00:01.00]Is this the real life?" },
                "code": 200
            })))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn returns_first_source_with_a_match() {
        let server = MockServer::start().await;
        mount_lrclib_miss(&server).await;
        mount_netease_match(&server).await;

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
