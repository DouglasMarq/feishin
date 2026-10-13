use feishin_core::queue::{QueueTrack, YouTubeTrack};
use feishin_storage::settings::YouTubeSettings;
use feishin_youtube::{YouTubeHit, YtDlp, thumbnail_url};

pub fn yt_dlp_for(settings: &YouTubeSettings) -> YtDlp {
    YtDlp::new(settings.yt_dlp_program()).with_cookies_from(settings.cookies_from_browser)
}

pub fn youtube_track_of(hit: &YouTubeHit) -> YouTubeTrack {
    YouTubeTrack {
        video_id: hit.video_id.clone(),
        title: hit.title.clone(),
        artists: hit.artist.iter().cloned().collect(),
        album: hit.album.clone(),
        thumbnail_url: Some(
            hit.thumbnail_url
                .clone()
                .unwrap_or_else(|| thumbnail_url(&hit.video_id)),
        ),
        duration_ms: hit.duration_ms.unwrap_or_default(),
    }
}

pub fn queue_track_of(hit: &YouTubeHit) -> QueueTrack {
    QueueTrack::from(&youtube_track_of(hit))
}

#[cfg(test)]
mod tests {
    use feishin_youtube::CookieBrowser;

    use super::*;

    fn hit() -> YouTubeHit {
        YouTubeHit {
            video_id: "abc".into(),
            title: "Song".into(),
            artist: Some("Singer".into()),
            album: None,
            thumbnail_url: None,
            duration_ms: Some(90_000),
        }
    }

    #[test]
    fn hits_become_queue_tracks_with_a_thumbnail() {
        let track = queue_track_of(&hit());
        assert_eq!(track.song_id, "yt:abc");
        assert_eq!(track.artist_names(), "Singer");
        assert_eq!(track.duration_ms, 90_000);
        assert_eq!(
            track.thumbnail_url(),
            Some("https://i.ytimg.com/vi/abc/hqdefault.jpg")
        );
    }

    #[test]
    fn yt_dlp_follows_the_settings() {
        let settings = YouTubeSettings {
            yt_dlp_path: Some("/opt/yt-dlp".into()),
            cookies_from_browser: Some(CookieBrowser::Brave),
            country: None,
        };
        assert_eq!(
            yt_dlp_for(&settings),
            YtDlp::new("/opt/yt-dlp").with_cookies_from(Some(CookieBrowser::Brave))
        );
    }
}
