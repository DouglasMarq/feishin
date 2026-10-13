use std::sync::Arc;

use super::{QueueTrack, TrackSource, YouTubeVideo};
use crate::model::{ArtistRef, SongId};

pub const YOUTUBE_SERVER_ID: &str = "youtube";
pub const YOUTUBE_SONG_ID_PREFIX: &str = "yt:";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct YouTubeTrack {
    pub video_id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub thumbnail_url: Option<String>,
    pub duration_ms: u64,
}

pub fn youtube_song_id(video_id: &str) -> String {
    format!("{YOUTUBE_SONG_ID_PREFIX}{video_id}")
}

fn artist_refs(names: &[String]) -> Arc<[ArtistRef]> {
    names
        .iter()
        .filter(|name| !name.trim().is_empty())
        .map(|name| ArtistRef {
            id: String::new(),
            name: name.clone(),
        })
        .collect()
}

impl From<&YouTubeTrack> for QueueTrack {
    fn from(track: &YouTubeTrack) -> Self {
        let video = YouTubeVideo {
            video_id: Box::from(track.video_id.as_str()),
            thumbnail_url: track.thumbnail_url.as_deref().map(Box::from),
        };
        Self {
            song_id: SongId::from(youtube_song_id(&track.video_id)),
            server_id: Arc::from(YOUTUBE_SERVER_ID),
            title: Box::from(track.title.as_str()),
            artists: artist_refs(&track.artists),
            album: track.album.as_deref().map(Arc::from),
            album_id: None,
            cover_art_id: None,
            duration_ms: track.duration_ms,
            source: TrackSource::YouTube(Arc::new(video)),
        }
    }
}

impl QueueTrack {
    pub fn is_external(&self) -> bool {
        !self.source.is_server()
    }

    pub fn server_song_id(&self) -> Option<&str> {
        self.source.is_server().then(|| self.song_id.as_str())
    }

    pub fn youtube_video(&self) -> Option<&YouTubeVideo> {
        self.source.youtube()
    }

    pub fn thumbnail_url(&self) -> Option<&str> {
        self.youtube_video()
            .and_then(|video| video.thumbnail_url.as_deref())
    }
}
