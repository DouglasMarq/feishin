use crate::client::RequestEncoding;
use crate::types::OpenSubsonicExtensions;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Extensions {
    pub form_post: bool,
    pub index_based_queue: bool,
    pub playback_report: bool,
    pub top_songs_by_artist_id: bool,
    pub transcoding: bool,
    pub song_lyrics: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayQueueMode {
    IndexBased,
    CurrentSongId,
}

impl Extensions {
    pub(crate) fn from_response(response: OpenSubsonicExtensions) -> Self {
        let mut extensions = Self::default();
        for extension in response.open_subsonic_extensions.unwrap_or_default() {
            match extension.name.as_str() {
                "formPost" => extensions.form_post = true,
                "indexBasedQueue" => extensions.index_based_queue = true,
                "playbackReport" => extensions.playback_report = true,
                "topSongsByArtistId" => extensions.top_songs_by_artist_id = true,
                "transcoding" => extensions.transcoding = true,
                "songLyrics" => extensions.song_lyrics = extension.versions,
                _ => {}
            }
        }
        extensions
    }

    pub(crate) fn request_encoding(&self) -> RequestEncoding {
        match self.form_post {
            true => RequestEncoding::FormPost,
            false => RequestEncoding::QueryString,
        }
    }

    pub(crate) fn play_queue_mode(&self) -> PlayQueueMode {
        match self.index_based_queue {
            true => PlayQueueMode::IndexBased,
            false => PlayQueueMode::CurrentSongId,
        }
    }

    pub(crate) fn supports_song_lyrics(&self) -> bool {
        !self.song_lyrics.is_empty()
    }

    pub(crate) fn supports_enhanced_lyrics(&self) -> bool {
        self.song_lyrics.contains(&2)
    }
}
