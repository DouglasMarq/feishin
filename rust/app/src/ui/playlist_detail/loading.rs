use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_core::model::{Playlist, Song};
use feishin_storage::cover_cache::DecodedImage;
use slint::{ComponentHandle, ModelRc, VecModel};

use super::detail::{SharedDetail, lock};
use super::rows::{header_info, song_rows};
use crate::ui::player::to_image;
use crate::ui::playlists::fetch_all;
use crate::ui::{AppState, AppWindow, Context, PlaylistDetailState, Route, on_ui, show_status};

const COVER_SIZE: u32 = 300;

struct LoadRequest {
    window: slint::Weak<AppWindow>,
    shared: SharedDetail,
    ctx: Context,
    playlist_id: String,
    generation: u64,
}

fn reset_view(window: &AppWindow) {
    let state = window.global::<PlaylistDetailState>();
    state.set_name("".into());
    state.set_comment("".into());
    state.set_info("".into());
    state.set_has_cover(false);
    state.set_cover(slint::Image::default());
    state.set_rows(ModelRc::default());
    state.set_selected_count(0);
    state.set_busy(false);
}

pub fn load(window: &AppWindow, ctx: &Context, shared: &SharedDetail, playlist_id: &str) {
    let generation = lock(shared).restart(playlist_id);
    reset_view(window);
    let state = window.global::<PlaylistDetailState>();
    let Some(server) = ctx.session.get().filter(|_| !playlist_id.is_empty()) else {
        state.set_loading(false);
        return;
    };
    state.set_loading(true);
    let request = LoadRequest {
        window: window.as_weak(),
        shared: Arc::clone(shared),
        ctx: ctx.clone(),
        playlist_id: playlist_id.to_owned(),
        generation,
    };
    ctx.rt.spawn(fetch_playlist(server, request));
}

async fn fetch_playlist(server: Arc<dyn MusicServer>, request: LoadRequest) {
    let (songs, playlists) = tokio::join!(
        server.playlist_songs(&request.playlist_id),
        fetch_all(server.as_ref())
    );
    let metadata = playlists
        .inspect_err(|error| tracing::debug!("playlist metadata: {error}"))
        .ok()
        .and_then(|playlists| {
            playlists
                .into_iter()
                .find(|playlist| playlist.id == request.playlist_id)
        });
    if let Some(cover_id) = metadata
        .as_ref()
        .and_then(|playlist| playlist.cover_art_id.clone())
    {
        spawn_cover_load(&request, Arc::clone(&server), cover_id);
    }
    let LoadRequest {
        window,
        shared,
        generation,
        ..
    } = request;
    on_ui(&window, move |window| {
        apply_loaded(window, &shared, generation, songs, metadata);
    });
}

fn spawn_cover_load(request: &LoadRequest, server: Arc<dyn MusicServer>, cover_id: String) {
    let covers = request.ctx.covers.clone();
    let window = request.window.clone();
    let shared = Arc::clone(&request.shared);
    let generation = request.generation;
    request.ctx.rt.spawn(async move {
        let Ok(cover) = covers
            .cover(server.as_ref(), &cover_id, COVER_SIZE)
            .await
            .inspect_err(|error| tracing::debug!("playlist cover {cover_id}: {error:#}"))
        else {
            return;
        };
        on_ui(&window, move |window| {
            show_cover(window, &shared, generation, &cover);
        });
    });
}

fn show_cover(window: &AppWindow, shared: &SharedDetail, generation: u64, cover: &DecodedImage) {
    if lock(shared).generation != generation {
        return;
    }
    let Some(image) = to_image(cover) else {
        return;
    };
    let state = window.global::<PlaylistDetailState>();
    state.set_cover(image);
    state.set_has_cover(true);
}

fn apply_loaded(
    window: &AppWindow,
    shared: &SharedDetail,
    generation: u64,
    songs: feishin_api::Result<Vec<Song>>,
    metadata: Option<Playlist>,
) {
    let mut detail = lock(shared);
    if detail.generation != generation {
        return;
    }
    let state = window.global::<PlaylistDetailState>();
    state.set_loading(false);
    let songs = match songs {
        Ok(songs) => songs,
        Err(error) => {
            drop(detail);
            show_status(window, &format!("Could not load the playlist: {error}"));
            return;
        }
    };
    detail.name = metadata
        .as_ref()
        .map_or_else(|| "Playlist".to_owned(), |playlist| playlist.name.clone());
    show_header(window, &detail.name, &songs, metadata.as_ref());
    state.set_rows(ModelRc::new(VecModel::from(song_rows(&songs))));
    detail.songs = songs;
}

fn show_header(window: &AppWindow, name: &str, songs: &[Song], metadata: Option<&Playlist>) {
    let state = window.global::<PlaylistDetailState>();
    state.set_name(name.into());
    state.set_comment(
        metadata
            .and_then(|playlist| playlist.comment.as_deref())
            .unwrap_or_default()
            .trim()
            .into(),
    );
    state.set_info(
        header_info(
            songs,
            metadata.and_then(|playlist| playlist.owner.as_deref()),
        )
        .into(),
    );
}

pub fn refresh_if_showing(window: &AppWindow, playlist_id: &str) {
    let app = window.global::<AppState>();
    if app.get_route() == Route::PlaylistDetail && app.get_detail_id() == playlist_id {
        window
            .global::<PlaylistDetailState>()
            .invoke_load(playlist_id.into());
    }
}
