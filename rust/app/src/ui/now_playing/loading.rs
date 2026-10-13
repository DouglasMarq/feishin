use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_core::model::Song;
use slint::{ComponentHandle, ModelRc, VecModel};

use super::fetching::{fetch_cover, fetch_lyrics, fetch_similar};
use super::view::View;
use crate::ui::{AppWindow, ArtistLink, Context, LyricsStatus, NowPlayingState, on_ui};

pub(super) fn is_current(window: &AppWindow, song_id: &str) -> bool {
    window.global::<NowPlayingState>().get_song_id() == song_id
}

fn is_already_shown(state: &NowPlayingState, song_id: &str) -> bool {
    state.get_song_id() == song_id && (state.get_loaded() || song_id.is_empty())
}

pub(super) fn load(window: &AppWindow, ctx: &Context, view: &View, song_id: &str) {
    view.refresh_up_next(window);
    let state = window.global::<NowPlayingState>();
    if is_already_shown(&state, song_id) {
        return;
    }
    view.clear(window, song_id);
    let Some(server) = ctx.session.get().filter(|_| !song_id.is_empty()) else {
        return;
    };

    let cached = view.cached_lyrics(song_id);
    match &cached {
        Some(lyrics) => view.show_lyrics(window, lyrics.as_deref()),
        None => state.set_lyrics_status(LyricsStatus::Loading),
    }
    state.set_similar_loading(true);

    let request = DetailsRequest {
        weak_window: window.as_weak(),
        ctx: ctx.clone(),
        server,
        song_id: song_id.to_owned(),
        want_lyrics: cached.is_none(),
    };
    request.spawn();
}

struct DetailsRequest {
    weak_window: slint::Weak<AppWindow>,
    ctx: Context,
    server: Arc<dyn MusicServer>,
    song_id: String,
    want_lyrics: bool,
}

impl DetailsRequest {
    fn spawn(self) {
        let runtime = self.ctx.rt.clone();
        runtime.spawn(self.run());
    }

    async fn run(self) {
        let Self {
            weak_window,
            ctx,
            server,
            song_id,
            want_lyrics,
        } = self;
        let song = match server.song(&song_id).await {
            Ok(song) => song,
            Err(error) => {
                tracing::warn!("now playing {song_id}: {error}");
                show_details_failed(&weak_window, song_id);
                return;
            }
        };
        show_song(&weak_window, &song);
        tokio::join!(
            fetch_cover(&weak_window, &ctx, server.as_ref(), &song),
            fetch_similar(&weak_window, server.as_ref(), &song_id),
            async {
                if want_lyrics {
                    fetch_lyrics(&weak_window, &ctx, server.as_ref(), &song).await;
                }
            },
        );
    }
}

fn show_details_failed(weak_window: &slint::Weak<AppWindow>, song_id: String) {
    on_ui(weak_window, move |window| {
        if !is_current(window, &song_id) {
            return;
        }
        let state = window.global::<NowPlayingState>();
        state.set_similar_loading(false);
        if state.get_lyrics_status() == LyricsStatus::Loading {
            state.set_lyrics_status(LyricsStatus::Failed);
        }
    });
}

fn artist_links(song: &Song) -> Vec<ArtistLink> {
    song.artists
        .iter()
        .map(|artist| ArtistLink {
            id: artist.id.as_str().into(),
            name: artist.name.as_str().into(),
        })
        .collect()
}

fn show_song(weak_window: &slint::Weak<AppWindow>, song: &Song) {
    let song_id = song.id.clone();
    let artists = artist_links(song);
    let album_id = song.album_id.clone().unwrap_or_default();
    on_ui(weak_window, move |window| {
        if !is_current(window, &song_id) {
            return;
        }
        let state = window.global::<NowPlayingState>();
        state.set_artists(ModelRc::new(VecModel::from(artists)));
        state.set_album_id(album_id.into());
        state.set_loaded(true);
    });
}
