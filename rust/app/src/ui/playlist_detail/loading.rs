use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_core::model::{Page, Playlist, Song};
use feishin_storage::cover_cache::DecodedImage;
use slint::ComponentHandle;

use super::binding::with_model;
use super::detail::{SharedDetail, lock};
use super::pages::paging_for_page;
use super::rows::{HeaderSummary, header_info};
use crate::ui::player::to_image;
use crate::ui::{AppState, AppWindow, Context, PlaylistDetailState, Route, on_ui, show_status};

const COVER_SIZE: u32 = 300;

struct LoadRequest {
    window: slint::Weak<AppWindow>,
    shared: SharedDetail,
    ctx: Context,
    playlist_id: String,
    generation: u64,
}

struct Loaded {
    generation: u64,
    first_page: feishin_api::Result<Page<Song>>,
    metadata: Option<Playlist>,
}

fn reset_view(window: &AppWindow, generation: u64, playlist_id: &str) {
    let state = window.global::<PlaylistDetailState>();
    state.set_name("".into());
    state.set_comment("".into());
    state.set_info("".into());
    state.set_has_cover(false);
    state.set_cover(slint::Image::default());
    state.set_selected_count(0);
    state.set_busy(false);
    with_model(window, |model| model.reset(generation, playlist_id));
}

pub fn load(window: &AppWindow, ctx: &Context, shared: &SharedDetail, playlist_id: &str) {
    let generation = lock(shared).restart(playlist_id);
    reset_view(window, generation, playlist_id);
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
    let (metadata, first_page) = tokio::join!(
        server.playlist(&request.playlist_id),
        server.playlist_song_page(&request.playlist_id, paging_for_page(0))
    );
    let metadata = metadata
        .inspect_err(|error| tracing::debug!("playlist metadata: {error}"))
        .ok();
    if let Some(cover_id) = metadata
        .as_ref()
        .and_then(|playlist| playlist.cover_art_id.clone())
    {
        spawn_cover_load(&request, Arc::clone(&server), cover_id);
    }
    let loaded = Loaded {
        generation: request.generation,
        first_page,
        metadata,
    };
    let shared = request.shared;
    on_ui(&request.window, move |window| {
        apply_loaded(window, &shared, loaded);
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

fn apply_loaded(window: &AppWindow, shared: &SharedDetail, loaded: Loaded) {
    let mut detail = lock(shared);
    if detail.generation != loaded.generation {
        return;
    }
    window.global::<PlaylistDetailState>().set_loading(false);
    let page = match loaded.first_page {
        Ok(page) => page,
        Err(error) => {
            drop(detail);
            show_status(window, &format!("Could not load the playlist: {error}"));
            return;
        }
    };
    let metadata = loaded.metadata.as_ref();
    detail.name = metadata.map_or_else(|| "Playlist".to_owned(), |playlist| playlist.name.clone());
    let name = detail.name.clone();
    drop(detail);
    let total = page
        .total_count
        .or_else(|| metadata.map(|playlist| playlist.song_count as usize))
        .unwrap_or(page.items.len());
    show_header(
        window,
        &name,
        &HeaderSummary::new(metadata, total),
        metadata,
    );
    with_model(window, |model| {
        model.apply_first_page(loaded.generation, total, page.items)
    });
}

fn show_header(
    window: &AppWindow,
    name: &str,
    summary: &HeaderSummary,
    metadata: Option<&Playlist>,
) {
    let state = window.global::<PlaylistDetailState>();
    state.set_name(name.into());
    state.set_comment(
        metadata
            .and_then(|playlist| playlist.comment.as_deref())
            .unwrap_or_default()
            .trim()
            .into(),
    );
    state.set_info(header_info(summary).into());
}

pub fn refresh_if_showing(window: &AppWindow, playlist_id: &str) {
    let app = window.global::<AppState>();
    if app.get_route() == Route::PlaylistDetail && app.get_detail_id() == playlist_id {
        window
            .global::<PlaylistDetailState>()
            .invoke_load(playlist_id.into());
    }
}
