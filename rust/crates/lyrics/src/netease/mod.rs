use serde::Deserialize;

use crate::fuzzy::{MATCH_THRESHOLD, SearchItem, order_search_results};
use crate::http::send_json;
use crate::lrc::lyrics_from_text;
use crate::{Lyrics, LyricsQuery, LyricsSource, Result};

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

impl Song {
    fn joined_artist_names(&self) -> String {
        self.artists
            .iter()
            .flatten()
            .map(|artist| artist.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
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
    if query.has_no_search_terms() {
        return Ok(None);
    }
    let songs = search_songs(client, base, query).await?;
    let artists: Vec<String> = songs.iter().map(Song::joined_artist_names).collect();
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
        let Some(song) = songs.get(ranked.index) else {
            continue;
        };
        if ranked.score > MATCH_THRESHOLD {
            continue;
        }
        let Some(lyrics) = lyrics_by_id(client, base, song.id).await? else {
            continue;
        };
        if lyrics.synced {
            return Ok(Some(lyrics));
        }
        fallback.get_or_insert(lyrics);
    }
    Ok(fallback)
}

async fn search_songs(
    client: &reqwest::Client,
    base: &str,
    query: &LyricsQuery,
) -> Result<Vec<Song>> {
    let search_query = format!("{} {}", query.artist, query.title);
    let request = client.get(format!("{base}/api/search/get")).query(&[
        ("limit", "5"),
        ("offset", "0"),
        ("s", search_query.as_str()),
        ("type", "1"),
    ]);
    Ok(send_json::<SearchResponse>(request)
        .await?
        .and_then(|response| response.result)
        .and_then(|result| result.songs)
        .unwrap_or_default())
}

async fn lyrics_by_id(
    client: &reqwest::Client,
    base: &str,
    song_id: u64,
) -> Result<Option<Lyrics>> {
    let request = client.get(format!("{base}/api/song/lyric")).query(&[
        ("id", song_id.to_string().as_str()),
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
mod tests;
