use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_core::model::{Page, Song, SongListQuery};
use slint::ComponentHandle;

use super::model::{Request, paging_for_page};
use super::with_model;
use crate::ui::{AppWindow, Context, TracksState, clamp_i32, on_ui, show_status};

pub(super) fn spawn_request(window: slint::Weak<AppWindow>, ctx: &Context, request: Request) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    ctx.rt.spawn(async move {
        match request {
            Request::Count { generation, query } => {
                load_count(server, window, generation, query).await;
            }
            Request::Page {
                generation,
                query,
                page,
            } => load_page(server, window, generation, query, page).await,
        }
    });
}

async fn load_count(
    server: Arc<dyn MusicServer>,
    window: slint::Weak<AppWindow>,
    generation: u64,
    query: SongListQuery,
) {
    let result = server.song_count(&query).await;
    on_ui(&window, move |window| {
        show_count(window, generation, result)
    });
}

fn show_count(window: &AppWindow, generation: u64, result: feishin_api::Result<usize>) {
    let total = result.as_ref().map_or(0, |total| *total);
    let applied = with_model(window, |model| model.apply_count(generation, total));
    if applied != Some(true) {
        return;
    }
    let state = window.global::<TracksState>();
    state.set_total(clamp_i32(total as u64));
    state.set_loading(false);
    if let Err(error) = result {
        show_status(window, &format!("Could not load tracks: {error}"));
    }
}

async fn load_page(
    server: Arc<dyn MusicServer>,
    window: slint::Weak<AppWindow>,
    generation: u64,
    query: SongListQuery,
    page: usize,
) {
    let result = server.song_list(&query, paging_for_page(page)).await;
    on_ui(&window, move |window| {
        show_page(window, generation, page, result);
    });
}

fn show_page(
    window: &AppWindow,
    generation: u64,
    page: usize,
    result: feishin_api::Result<Page<Song>>,
) {
    match result {
        Ok(songs) => {
            with_model(window, |model| {
                model.apply_page(generation, page, songs.items);
            });
        }
        Err(error) => {
            with_model(window, |model| model.page_failed(generation, page));
            show_status(window, &format!("Could not load tracks: {error}"));
        }
    }
}
