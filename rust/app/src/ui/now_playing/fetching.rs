use std::rc::Rc;

use feishin_api::MusicServer;
use feishin_core::model::Song;
use feishin_storage::settings::IntegrationSettings;
use slint::{ComponentHandle, ModelRc, VecModel};

use super::loading::is_current;
use super::lyrics_plan::lyrics_plan;
use super::song_lyrics::{LyricsLookupError, SongLyrics, resolve_lyrics};
use super::view::{View, with_view};
use crate::ui::{
    AppWindow, Context, LyricsStatus, NowPlayingState, TrackRow, on_ui, player, tracks,
};

const COVER_SIZE: u32 = 600;
const SIMILAR_COUNT: usize = 20;

pub(super) async fn fetch_cover(
    weak_window: &slint::Weak<AppWindow>,
    ctx: &Context,
    server: &dyn MusicServer,
    song: &Song,
) {
    let Some(cover_id) = song.cover_art_id.as_deref() else {
        return;
    };
    let cover = match ctx.covers.cover(server, cover_id, COVER_SIZE).await {
        Ok(cover) => cover,
        Err(error) => {
            tracing::debug!("cover {cover_id}: {error:#}");
            return;
        }
    };
    let song_id = song.id.clone();
    on_ui(weak_window, move |window| {
        if is_current(window, &song_id)
            && let Some(image) = player::to_image(&cover)
        {
            let state = window.global::<NowPlayingState>();
            state.set_cover(image);
            state.set_has_cover(true);
        }
    });
}

pub(super) async fn fetch_similar(
    weak_window: &slint::Weak<AppWindow>,
    server: &dyn MusicServer,
    song_id: &str,
) {
    let songs = server
        .similar_songs(song_id, SIMILAR_COUNT)
        .await
        .unwrap_or_else(|error| {
            tracing::debug!("similar songs for {song_id}: {error}");
            Vec::new()
        });
    let song_id = song_id.to_owned();
    on_ui(weak_window, move |window| {
        if !is_current(window, &song_id) {
            return;
        }
        let rows: Vec<TrackRow> = songs
            .iter()
            .enumerate()
            .map(|(index, song)| tracks::track_row(song, index))
            .collect();
        let state = window.global::<NowPlayingState>();
        state.set_similar(ModelRc::new(VecModel::from(rows)));
        state.set_similar_loading(false);
        with_view(|view| *view.similar.borrow_mut() = songs);
    });
}

fn configured_lyrics_sources(ctx: &Context) -> Vec<String> {
    match ctx.store.load_settings() {
        Ok(settings) => settings.integrations.lyrics_sources,
        Err(error) => {
            tracing::debug!("settings: {error:#}");
            IntegrationSettings::default().lyrics_sources
        }
    }
}

pub(super) async fn fetch_lyrics(
    weak_window: &slint::Weak<AppWindow>,
    ctx: &Context,
    server: &dyn MusicServer,
    song: &Song,
) {
    let source_names = configured_lyrics_sources(ctx);
    let result = resolve_lyrics(server, song, &lyrics_plan(&source_names)).await;
    let song_id = song.id.clone();
    on_ui(weak_window, move |window| {
        with_view(|view| apply_lyrics_result(window, view, &song_id, result));
    });
}

fn apply_lyrics_result(
    window: &AppWindow,
    view: &View,
    song_id: &str,
    result: Result<Option<SongLyrics>, LyricsLookupError>,
) {
    let lyrics = match result {
        Ok(lyrics) => lyrics.map(Rc::new),
        Err(error) => {
            tracing::warn!("lyrics for {song_id}: {error}");
            if is_current(window, song_id) {
                window
                    .global::<NowPlayingState>()
                    .set_lyrics_status(LyricsStatus::Failed);
            }
            return;
        }
    };
    view.remember(song_id.to_owned(), lyrics.clone());
    if is_current(window, song_id) {
        view.show_lyrics(window, lyrics.as_deref());
    }
}
