mod page;

use serde::Deserialize;

use crate::fuzzy::{MATCH_THRESHOLD, SearchItem, order_search_results};
use crate::http::{send_json, send_text};
use crate::lrc::lyrics_from_text;
use crate::{Lyrics, LyricsQuery, LyricsSource, Result};
use page::extract_lyrics;

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
    if query.has_no_search_terms() {
        return Ok(None);
    }
    let songs = search_songs(client, base, query).await?;
    let items: Vec<SearchItem<'_>> = songs
        .iter()
        .map(|song| SearchItem {
            name: &song.full_title,
            artist: &song.artist_names,
            is_sync: None,
        })
        .collect();
    for ranked in order_search_results(&query.title, &query.artist, &items) {
        let Some(song) = songs.get(ranked.index) else {
            continue;
        };
        if ranked.score > MATCH_THRESHOLD {
            continue;
        }
        if let Some(lyrics) = scrape_song_page(client, &song.url).await? {
            return Ok(Some(lyrics));
        }
    }
    Ok(None)
}

async fn search_songs(
    client: &reqwest::Client,
    base: &str,
    query: &LyricsQuery,
) -> Result<Vec<Song>> {
    let search_query = format!("{} {}", query.artist, query.title);
    let request = client
        .get(format!("{base}/api/search/song"))
        .query(&[("per_page", "5"), ("q", search_query.as_str())]);
    let songs = send_json::<SearchResponse>(request)
        .await?
        .and_then(|response| response.response)
        .and_then(|body| body.sections)
        .and_then(|sections| sections.into_iter().next())
        .and_then(|section| section.hits)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|hit| hit.result)
        .collect();
    Ok(songs)
}

async fn scrape_song_page(client: &reqwest::Client, url: &str) -> Result<Option<Lyrics>> {
    let Some(html) = send_text(client.get(url)).await? else {
        return Ok(None);
    };
    Ok(lyrics_from_text(
        LyricsSource::Genius,
        &extract_lyrics(&html),
    ))
}

#[cfg(test)]
mod tests;
