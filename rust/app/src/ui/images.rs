use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_core::queue::{QueueTrack, YouTubeVideo};
use feishin_storage::cover_cache::DecodedImage;
use slint::{Color, Image, Rgba8Pixel, SharedPixelBuffer};

use crate::covers::CoverProvider;

const RGBA_BYTES_PER_PIXEL: usize = 4;

pub fn to_image(cover: &DecodedImage) -> Option<Image> {
    let expected_len = cover.width as usize * cover.height as usize * RGBA_BYTES_PER_PIXEL;
    if cover.rgba.len() != expected_len || expected_len == 0 {
        return None;
    }
    let buffer =
        SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&cover.rgba, cover.width, cover.height);
    Some(Image::from_rgba8(buffer))
}

pub fn tint_of(cover: &DecodedImage) -> Option<Color> {
    cover
        .dominant_tint()
        .map(|tint| Color::from_rgb_u8(tint.red, tint.green, tint.blue))
}

fn thumbnail_cache_key(video: &YouTubeVideo) -> String {
    format!("youtube:{}", video.video_id)
}

pub enum CoverSource {
    Server {
        server: Arc<dyn MusicServer>,
        cover_id: String,
    },
    Thumbnail {
        cache_key: String,
        url: String,
    },
}

impl CoverSource {
    pub fn of_track(track: &QueueTrack, server: Option<Arc<dyn MusicServer>>) -> Option<Self> {
        match track.youtube_video() {
            Some(video) => Self::thumbnail(video),
            None => Some(Self::Server {
                server: server?,
                cover_id: track.cover_art_id.as_deref()?.to_owned(),
            }),
        }
    }

    pub fn thumbnail(video: &YouTubeVideo) -> Option<Self> {
        Some(Self::Thumbnail {
            cache_key: thumbnail_cache_key(video),
            url: video.thumbnail_url.as_deref()?.to_owned(),
        })
    }

    pub async fn fetch(&self, covers: &dyn CoverProvider, size: u32) -> Option<Arc<DecodedImage>> {
        match self {
            Self::Server { server, cover_id } => covers
                .cover(server.as_ref(), cover_id, size)
                .await
                .inspect_err(|error| tracing::debug!("cover {cover_id}: {error:#}"))
                .ok(),
            Self::Thumbnail { cache_key, url } => covers
                .cover_from_url(cache_key, url, size)
                .await
                .inspect_err(|error| tracing::debug!("thumbnail {url}: {error:#}"))
                .ok(),
        }
    }
}

#[cfg(test)]
mod tests {
    use feishin_core::queue::YouTubeTrack;

    use super::*;
    use crate::demo::{FakeServer, test_song};

    fn youtube_track(thumbnail_url: Option<&str>) -> QueueTrack {
        QueueTrack::from(&YouTubeTrack {
            video_id: "abc".into(),
            title: "Song".into(),
            artists: vec!["Singer".into()],
            album: None,
            thumbnail_url: thumbnail_url.map(Into::into),
            duration_ms: 0,
        })
    }

    fn server_track(cover_art_id: Option<&str>) -> QueueTrack {
        let mut song = test_song("song-1", 1_000);
        song.cover_art_id = cover_art_id.map(Into::into);
        QueueTrack::from(&song)
    }

    #[test]
    fn given_a_youtube_track_its_cover_is_the_cached_thumbnail() {
        let source = CoverSource::of_track(
            &youtube_track(Some("https://thumbnails.example/abc.jpg")),
            None,
        );
        assert!(matches!(
            source,
            Some(CoverSource::Thumbnail { cache_key, url })
                if cache_key == "youtube:abc" && url == "https://thumbnails.example/abc.jpg"
        ));
    }

    #[test]
    fn given_a_youtube_track_without_a_thumbnail_there_is_no_cover() {
        assert!(CoverSource::of_track(&youtube_track(None), None).is_none());
    }

    #[test]
    fn given_a_server_track_its_cover_comes_from_the_connected_server() {
        let server: Arc<dyn MusicServer> = Arc::new(FakeServer::new(1));
        let track = server_track(Some("cover-1"));
        assert!(CoverSource::of_track(&track, None).is_none());
        assert!(CoverSource::of_track(&server_track(None), Some(Arc::clone(&server))).is_none());
        assert!(matches!(
            CoverSource::of_track(&track, Some(server)),
            Some(CoverSource::Server { cover_id, .. }) if cover_id == "cover-1"
        ));
    }

    #[test]
    fn given_pixels_that_do_not_fill_the_image_no_image_is_made() {
        let cover = |rgba_len: usize| DecodedImage {
            width: 2,
            height: 2,
            rgba: vec![0; rgba_len],
        };
        assert!(to_image(&cover(16)).is_some());
        assert!(to_image(&cover(15)).is_none());
        assert!(to_image(&cover(0)).is_none());
    }

    #[test]
    fn given_a_colorful_cover_its_tint_becomes_a_color() {
        let cover = DecodedImage {
            width: 2,
            height: 2,
            rgba: [10, 200, 90, 255].repeat(4),
        };
        assert_eq!(tint_of(&cover), Some(Color::from_rgb_u8(10, 200, 90)));
    }

    #[test]
    fn given_a_grey_cover_there_is_no_tint() {
        let cover = DecodedImage {
            width: 2,
            height: 2,
            rgba: [120, 120, 120, 255].repeat(4),
        };
        assert_eq!(tint_of(&cover), None);
    }
}
