use serde::Deserialize;

use crate::is_video_id;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YouTubeHit {
    pub video_id: String,
    pub title: String,
    pub artist: Option<String>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct SearchEntry {
    #[serde(default)]
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    artists: Option<Vec<String>>,
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    uploader: Option<String>,
}

impl SearchEntry {
    fn into_hit(self) -> Option<YouTubeHit> {
        if !is_video_id(&self.id) || self.title.trim().is_empty() {
            return None;
        }
        let artist = self
            .artists
            .map(|artists| artists.join(", "))
            .filter(|artists| !artists.is_empty())
            .or(self.channel)
            .or(self.uploader);
        Some(YouTubeHit {
            video_id: self.id,
            title: self.title,
            artist,
            duration_ms: self.duration.and_then(seconds_to_ms),
        })
    }
}

fn seconds_to_ms(seconds: f64) -> Option<u64> {
    (seconds.is_finite() && seconds > 0.0).then(|| (seconds * 1000.0).round() as u64)
}

pub(crate) fn parse_search_output(stdout: &str) -> Vec<YouTubeHit> {
    stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| {
            serde_json::from_str::<SearchEntry>(line)
                .inspect_err(|error| tracing::debug!("skipping a yt-dlp search line: {error}"))
                .ok()
        })
        .filter_map(SearchEntry::into_hit)
        .collect()
}
