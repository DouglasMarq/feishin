use feishin_api::MusicServer;
use feishin_core::model::{Paging, Song, SongListQuery};
use slint::{ComponentHandle, ModelRc};

use super::binding::{Binding, with_table};
use super::track_table::TrackTable;
use crate::ui::tracks::{PAGE_SIZE, Request};
use crate::ui::{AppWindow, Context, TrackItem, on_ui, show_status};

pub fn track_table(
    window: &AppWindow,
    ctx: &Context,
    favorite: bool,
    binding: Binding<TrackItem>,
) -> ModelRc<TrackItem> {
    let weak = window.as_weak();
    let ctx = ctx.clone();
    ModelRc::new(TrackTable::new(favorite, move |request| {
        spawn_tracks(weak.clone(), &ctx, binding, request);
    }))
}

fn spawn_tracks(
    window: slint::Weak<AppWindow>,
    ctx: &Context,
    binding: Binding<TrackItem>,
    request: Request,
) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    ctx.rt.spawn(async move {
        match request {
            Request::Count { generation, query } => {
                let result = server.song_count(&query).await;
                on_ui(&window, move |window| {
                    apply_track_count(window, binding, generation, result);
                });
            }
            Request::Page {
                generation,
                query,
                page,
            } => {
                let result = fetch_track_page(server.as_ref(), &query, page).await;
                on_ui(&window, move |window| {
                    apply_track_page(window, binding, generation, page, result);
                });
            }
        }
    });
}

async fn fetch_track_page(
    server: &dyn MusicServer,
    query: &SongListQuery,
    page: usize,
) -> feishin_api::Result<Vec<Song>> {
    let paging = Paging {
        start_index: page * PAGE_SIZE,
        limit: PAGE_SIZE,
    };
    server
        .song_list(query, paging)
        .await
        .map(|songs| songs.items)
}

fn apply_track_count(
    window: &AppWindow,
    binding: Binding<TrackItem>,
    generation: u64,
    result: feishin_api::Result<usize>,
) {
    let total = result.as_ref().map_or(0, |total| *total);
    let applied = with_table(window, binding, |table| {
        table.apply_count(generation, total)
    });
    if applied != Some(true) {
        return;
    }
    (binding.loading)(window, false);
    if let Err(error) = result {
        show_status(window, &format!("Could not load tracks: {error}"));
    }
}

fn apply_track_page(
    window: &AppWindow,
    binding: Binding<TrackItem>,
    generation: u64,
    page: usize,
    result: feishin_api::Result<Vec<Song>>,
) {
    match result {
        Ok(songs) => {
            with_table(window, binding, |table| {
                table.apply_page(generation, page, songs);
            });
        }
        Err(error) => {
            with_table(window, binding, |table| table.page_failed(generation, page));
            show_status(window, &format!("Could not load tracks: {error}"));
        }
    }
}
