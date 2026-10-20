use feishin_core::queue::{QueueTrack, YouTubeVideo};
use feishin_lyrics::{LyricsQuery, LyricsSource};
use slint::{ComponentHandle, ModelRc, VecModel};

use super::fetching::{apply_lyrics_result, configured_lyrics_sources, show_cover};
use super::lyrics_plan::remote_sources;
use super::song_lyrics::remote_lyrics_for;
use super::view::{View, with_view};
use crate::session::Session;
use crate::ui::images::CoverSource;
use crate::ui::{AppWindow, ArtistLink, Context, LyricsStatus, NowPlayingState, on_ui};

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

fn external_lyrics_sources(session: &Session) -> Vec<LyricsSource> {
    remote_sources(&configured_lyrics_sources(session))
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
    let sources = external_lyrics_sources(&ctx.session);
    match view.cached_lyrics(&song_id) {
        Some(lyrics) => view.show_lyrics(window, lyrics.as_deref()),
        None if sources.is_empty() => view.show_lyrics(window, None),
        None => {
            state.set_lyrics_status(LyricsStatus::Loading);
            fetch_lyrics(
                window,
                ctx,
                LyricsRequest {
                    song_id,
                    query: lyrics_query(track),
                    sources,
                },
            );
        }
    }
}

fn fetch_thumbnail(window: &AppWindow, ctx: &Context, song_id: &str, video: &YouTubeVideo) {
    let Some(source) = CoverSource::thumbnail(video) else {
        return;
    };
    let (show_ctx, weak_window, song_id) = (ctx.clone(), window.as_weak(), song_id.to_owned());
    ctx.rt.spawn(async move {
        show_cover(&weak_window, &show_ctx, &source, song_id).await;
    });
}

struct LyricsRequest {
    song_id: String,
    query: LyricsQuery,
    sources: Vec<LyricsSource>,
}

fn fetch_lyrics(window: &AppWindow, ctx: &Context, request: LyricsRequest) {
    let weak_window = window.as_weak();
    let provider = ctx.connector.lyrics();
    ctx.rt.spawn(async move {
        let LyricsRequest {
            song_id,
            query,
            sources,
        } = request;
        let result = remote_lyrics_for(provider.as_ref(), &query, &sources).await;
        on_ui(&weak_window, move |window| {
            with_view(|view| apply_lyrics_result(window, view, &song_id, result));
        });
    });
}

#[cfg(test)]
mod tests {
    use feishin_core::queue::YouTubeTrack;
    use feishin_storage::settings::{LyricsSourceSetting, Settings};

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

    #[test]
    fn given_an_external_track_lyrics_sources_come_from_the_settings_snapshot() {
        let session = Session::default();
        let mut settings = Settings::default();
        settings.integrations.lyrics_sources =
            vec![LyricsSourceSetting::Server, LyricsSourceSetting::Genius];
        session.publish_settings(&settings);
        assert_eq!(
            external_lyrics_sources(&session),
            vec![LyricsSource::Genius]
        );
    }
}
