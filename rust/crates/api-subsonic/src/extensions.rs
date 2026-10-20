use crate::client::RequestEncoding;
use crate::types::OpenSubsonicExtensions;

const ENHANCED_SONG_LYRICS_VERSION: u32 = 2;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Extensions {
    pub has_form_post: bool,
    pub has_index_based_queue: bool,
    pub has_playback_report: bool,
    pub has_top_songs_by_artist_id: bool,
    pub has_transcoding: bool,
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
                "formPost" => extensions.has_form_post = true,
                "indexBasedQueue" => extensions.has_index_based_queue = true,
                "playbackReport" => extensions.has_playback_report = true,
                "topSongsByArtistId" => extensions.has_top_songs_by_artist_id = true,
                "transcoding" => extensions.has_transcoding = true,
                "songLyrics" => extensions.song_lyrics = extension.versions,
                _ => {}
            }
        }
        extensions
    }

    pub(crate) fn request_encoding(&self) -> RequestEncoding {
        match self.has_form_post {
            true => RequestEncoding::FormPost,
            false => RequestEncoding::QueryString,
        }
    }

    pub(crate) fn play_queue_mode(&self) -> PlayQueueMode {
        match self.has_index_based_queue {
            true => PlayQueueMode::IndexBased,
            false => PlayQueueMode::CurrentSongId,
        }
    }

    pub(crate) fn supports_song_lyrics(&self) -> bool {
        !self.song_lyrics.is_empty()
    }

    pub(crate) fn supports_enhanced_lyrics(&self) -> bool {
        self.song_lyrics.contains(&ENHANCED_SONG_LYRICS_VERSION)
    }
}
