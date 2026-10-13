use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use feishin_core::model::Paging;
use slint::ComponentHandle;

use super::super::player::to_image;
use super::super::{AppWindow, Context, on_ui, show_status};
use super::model::with_home_model;
use super::shelf::SHELF_SIZE;
use crate::controller::Command;

const COVER_SIZE: u32 = 192;

pub fn load_shelf(window: &AppWindow, ctx: &Context, shelf: usize) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let Some(Some((generation, query))) = with_home_model(window, |model| model.begin_fetch(shelf))
    else {
        return;
    };
    let weak_window = window.as_weak();
    ctx.rt.spawn(async move {
        let paging = Paging {
            start_index: 0,
            limit: SHELF_SIZE,
        };
        let result = server.album_list(&query, paging).await;
        on_ui(&weak_window, move |window| match result {
            Ok(page) => {
                with_home_model(window, |model| {
                    model.apply_fetched(shelf, generation, &page.items)
                });
            }
            Err(error) => {
                let is_current_fetch =
                    with_home_model(window, |model| model.fetch_failed(shelf, generation));
                if is_current_fetch == Some(true) {
                    show_status(window, &format!("Could not load albums: {error}"));
                }
            }
        });
    });
}

pub fn load_cover(window: &AppWindow, ctx: &Context, shelf: usize, index: usize) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let Some(Some((album_id, cover_id))) =
        with_home_model(window, |model| model.request_cover(shelf, index))
    else {
        return;
    };
    let covers = Arc::clone(&ctx.covers);
    let weak_window = window.as_weak();
    ctx.rt.spawn(async move {
        let result = covers.cover(server.as_ref(), &cover_id, COVER_SIZE).await;
        if let Err(error) = &result {
            tracing::debug!("cover {cover_id}: {error:#}");
        }
        on_ui(&weak_window, move |window| {
            let image = result.ok().and_then(|cover| to_image(&cover));
            with_home_model(window, |model| model.set_cover(shelf, &album_id, image));
        });
    });
}

pub fn play_album(
    window: &AppWindow,
    ctx: &Context,
    latest_request: &Arc<AtomicU64>,
    album_id: String,
) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let request = latest_request.fetch_add(1, Ordering::Relaxed) + 1;
    let latest_request = Arc::clone(latest_request);
    let ctx = ctx.clone();
    let weak_window = window.as_weak();
    ctx.rt.clone().spawn(async move {
        match server.album(&album_id).await {
            Ok(detail) if latest_request.load(Ordering::Relaxed) == request => {
                ctx.send(Command::PlaySongs {
                    songs: detail.songs,
                    song_id: None,
                });
            }
            Ok(_) => {}
            Err(error) => on_ui(&weak_window, move |window| {
                show_status(window, &format!("Could not play album: {error}"));
            }),
        }
    });
}
