use feishin_core::queue::{QueueTrack, YouTubeVideo};
use feishin_lyrics::LyricsQuery;
use slint::{ComponentHandle, ModelRc, VecModel};

use super::fetching::{apply_lyrics_result, configured_lyrics_sources};
use super::loading::is_current;
use super::lyrics_plan::remote_sources;
use super::song_lyrics::remote_lyrics_for;
use super::view::{View, with_view};
use crate::ui::player::{thumbnail_cache_key, to_image};
use crate::ui::{AppWindow, ArtistLink, Context, LyricsStatus, NowPlayingState, on_ui};

const COVER_SIZE: u32 = 600;

fn lyrics_query(track: &QueueTrack) -> LyricsQuery {
    LyricsQuery {
        title: track.title.to_string(),
        artist: track.artist_names(),
        album: track.album.as_deref().map(str::to_owned),
        duration_ms: Some(track.duration_ms).filter(|duration_ms| *duration_ms > 0),
    }
}

fn artist_links(track: &QueueTrack) -> Vec<ArtistLink> {
    track
        .artists
        .iter()
        .map(|artist| ArtistLink {
            id: "".into(),
            name: artist.name.as_str().into(),
        })
        .collect()
}

pub(super) fn load_external(window: &AppWindow, ctx: &Context, view: &View, track: &QueueTrack) {
    let state = window.global::<NowPlayingState>();
    state.set_external(true);
    state.set_artists(ModelRc::new(VecModel::from(artist_links(track))));
    state.set_loaded(true);
    let song_id = track.song_id.to_string();
    if let Some(video) = track.youtube_video() {
        fetch_thumbnail(window, ctx, &song_id, video);
    }
    match view.cached_lyrics(&song_id) {
        Some(lyrics) => view.show_lyrics(window, lyrics.as_deref()),
        None => {
            state.set_lyrics_status(LyricsStatus::Loading);
            fetch_lyrics(window, ctx, song_id, lyrics_query(track));
        }
    }
}

fn fetch_thumbnail(window: &AppWindow, ctx: &Context, song_id: &str, video: &YouTubeVideo) {
    let Some(url) = video.thumbnail_url.as_deref().map(str::to_owned) else {
        return;
    };
    let cache_key = thumbnail_cache_key(video);
    let (covers, weak_window, song_id) = (ctx.covers.clone(), window.as_weak(), song_id.to_owned());
    ctx.rt.spawn(async move {
        let cover = match covers.cover_from_url(&cache_key, &url, COVER_SIZE).await {
            Ok(cover) => cover,
            Err(error) => {
                tracing::debug!("thumbnail {url}: {error:#}");
                return;
            }
        };
        on_ui(&weak_window, move |window| {
            if is_current(window, &song_id)
                && let Some(image) = to_image(&cover)
            {
                let state = window.global::<NowPlayingState>();
                state.set_cover(image);
                state.set_has_cover(true);
            }
        });
    });
}

fn fetch_lyrics(window: &AppWindow, ctx: &Context, song_id: String, query: LyricsQuery) {
    let sources = remote_sources(&configured_lyrics_sources(ctx));
    let weak_window = window.as_weak();
    ctx.rt.spawn(async move {
        let result = remote_lyrics_for(&query, &sources).await;
        on_ui(&weak_window, move |window| {
            with_view(|view| apply_lyrics_result(window, view, &song_id, result));
        });
    });
}

#[cfg(test)]
mod tests {
    use feishin_core::queue::YouTubeTrack;

    use super::*;

    #[test]
    fn lyrics_are_searched_by_title_and_artist() {
        let track = QueueTrack::from(&YouTubeTrack {
            video_id: "abc".into(),
            title: "Song".into(),
            artists: vec!["Singer".into(), "Guest".into()],
            album: None,
            thumbnail_url: None,
            duration_ms: 0,
        });
        let query = lyrics_query(&track);
        assert_eq!(query.title, "Song");
        assert_eq!(query.artist, "Singer, Guest");
        assert_eq!(query.duration_ms, None);
        assert!(artist_links(&track).iter().all(|link| link.id.is_empty()));
    }
}
