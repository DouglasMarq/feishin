use serde::Deserialize;

use crate::fuzzy::{SearchItem, order_search_results};
use crate::{
    Lyrics, LyricsQuery, LyricsSource, MATCH_THRESHOLD, Result, lyrics_from_text, send_json,
};

#[derive(Debug, Deserialize)]
struct SearchResponse {
    result: Option<SearchResult>,
}

#[derive(Debug, Deserialize)]
struct SearchResult {
    songs: Option<Vec<Song>>,
}

#[derive(Debug, Deserialize)]
struct Song {
    id: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    artists: Option<Vec<Artist>>,
}

#[derive(Debug, Deserialize)]
struct Artist {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct LyricResponse {
    lrc: Option<LyricBody>,
}

#[derive(Debug, Deserialize)]
struct LyricBody {
    lyric: Option<String>,
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
    let request = client.get(format!("{base}/api/search/get")).query(&[
        ("limit", "5"),
        ("offset", "0"),
        ("s", search_query.as_str()),
        ("type", "1"),
    ]);
    let songs = send_json::<SearchResponse>(request)
        .await?
        .and_then(|response| response.result)
        .and_then(|result| result.songs)
        .unwrap_or_default();

    let artists: Vec<String> = songs
        .iter()
        .map(|song| {
            song.artists
                .iter()
                .flatten()
                .map(|artist| artist.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .collect();
    let items: Vec<SearchItem<'_>> = songs
        .iter()
        .zip(&artists)
        .map(|(song, artist)| SearchItem {
            name: &song.name,
            artist,
            is_sync: None,
        })
        .collect();

    let mut fallback = None;
    for ranked in order_search_results(&query.title, &query.artist, &items) {
        if ranked.score > MATCH_THRESHOLD {
            continue;
        }
        let Some(lyrics) = lyrics_by_id(client, base, songs[ranked.index].id).await? else {
            continue;
        };
        if lyrics.synced {
            return Ok(Some(lyrics));
        }
        fallback.get_or_insert(lyrics);
    }
    Ok(fallback)
}

async fn lyrics_by_id(client: &reqwest::Client, base: &str, id: u64) -> Result<Option<Lyrics>> {
    let request = client.get(format!("{base}/api/song/lyric")).query(&[
        ("id", id.to_string().as_str()),
        ("kv", "-1"),
        ("lv", "-1"),
        ("tv", "-1"),
    ]);
    let text = send_json::<LyricResponse>(request)
        .await?
        .and_then(|response| response.lrc)
        .and_then(|lrc| lrc.lyric);
    Ok(text.and_then(|text| lyrics_from_text(LyricsSource::NetEase, &text)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn query() -> LyricsQuery {
        LyricsQuery {
            title: "Lemon".to_owned(),
            artist: "Kenshi Yonezu".to_owned(),
            album: None,
            duration_ms: Some(255_000),
        }
    }

    fn song(id: u64, name: &str, artists: &[&str]) -> Value {
        json!({
            "id": id,
            "name": name,
            "artists": artists.iter().enumerate().map(|(i, a)| json!({
                "id": i, "name": a, "picUrl": null, "alias": [], "albumSize": 0,
                "picId": 0, "fansGroup": null, "img1v1Url": "", "img1v1": 0, "trans": null
            })).collect::<Vec<_>>(),
            "album": { "id": 9, "name": "Lemon", "artist": { "id": 0, "name": "" },
                       "publishTime": 0, "size": 1, "copyrightId": 0, "status": 0, "picId": 0, "mark": 0 },
            "duration": 255_000,
            "copyrightId": 0, "status": 0, "alias": [], "rtype": 0, "ftype": 0,
            "mvid": 0, "fee": 0, "rUrl": null, "mark": 0
        })
    }

    async fn mount_search(server: &MockServer, songs: Vec<Value>) {
        Mock::given(method("GET"))
            .and(path("/api/search/get"))
            .and(query_param("s", "Kenshi Yonezu Lemon"))
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
                song(111, "Lemon", &["Kenshi Yonezu"]),
                song(222, "Something Else", &["Other Person"]),
            ],
        )
        .await;
        mount_lyric(
            &server,
            "111",
            "[by:someone]\n[00:00.00] 作词 : 米津玄師\n[00:12.345]夢ならばどれほどよかったでしょう\n",
        )
        .await;

        let lyrics = fetch(&reqwest::Client::new(), &server.uri(), &query())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(lyrics.source, LyricsSource::NetEase);
        assert!(lyrics.synced);
        assert_eq!(lyrics.lines.len(), 2);
        assert_eq!(lyrics.lines[1].time_ms, Some(12_345));
        assert_eq!(lyrics.lines[1].text, "夢ならばどれほどよかったでしょう");
    }

    #[tokio::test]
    async fn prefers_a_synced_candidate_over_plain_one() {
        let server = MockServer::start().await;
        mount_search(
            &server,
            vec![
                song(111, "Lemon", &["Kenshi Yonezu"]),
                song(333, "Lemon (Live)", &["Kenshi Yonezu", "Guest"]),
            ],
        )
        .await;
        mount_lyric(&server, "111", "plain words only").await;
        mount_lyric(&server, "333", "[00:01.00]synced").await;

        let lyrics = fetch(&reqwest::Client::new(), &server.uri(), &query())
            .await
            .unwrap()
            .unwrap();
        assert!(lyrics.synced);
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
        mount_search(&server, vec![song(111, "Lemon", &["Kenshi Yonezu"])]).await;
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
}
