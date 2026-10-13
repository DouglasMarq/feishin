use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_core::model::Song;
use feishin_core::queue::{QueueTrack, YouTubeTrack};
use feishin_youtube::music::MusicSong;
use feishin_youtube::thumbnail_url;

use super::library_match::LibraryMatcher;

pub fn youtube_track_of(song: &MusicSong) -> YouTubeTrack {
    YouTubeTrack {
        video_id: song.video_id.clone(),
        title: song.title.clone(),
        artists: song.artists.clone(),
        album: song.album.clone(),
        thumbnail_url: Some(
            song.thumbnail_url
                .clone()
                .unwrap_or_else(|| thumbnail_url(&song.video_id)),
        ),
        duration_ms: song.duration_ms.unwrap_or_default(),
    }
}

pub fn queue_track_for(song: &MusicSong, library_song: Option<&Song>) -> QueueTrack {
    match library_song {
        Some(library_song) => QueueTrack::from(library_song),
        None => QueueTrack::from(&youtube_track_of(song)),
    }
}

pub async fn resolve_tracks(
    matcher: &LibraryMatcher,
    server: Option<Arc<dyn MusicServer>>,
    songs: &[MusicSong],
) -> Vec<QueueTrack> {
    let matches = match server {
        Some(server) => matcher.matches(server, songs).await,
        None => vec![None; songs.len()],
    };
    songs
        .iter()
        .zip(matches)
        .map(|(song, matched)| queue_track_for(song, matched.as_ref()))
        .collect()
}
