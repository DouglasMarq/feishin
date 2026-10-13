use std::sync::Arc;

use feishin_core::model::Paging;
use feishin_storage::cover_cache::DecodedImage;
use slint::{ComponentHandle, ModelRc};

use super::binding::{Binding, with_grid};
use super::card::CardPage;
use super::card_grid::CardGrid;
use super::grid_state::{GRID_PAGE, GridRequest};
use crate::ui::player::to_image;
use crate::ui::{AppWindow, CardRow, Context, on_ui, show_status};

const GRID_COVER_SIZE: u32 = 200;

pub fn card_grid(window: &AppWindow, ctx: &Context, binding: Binding<CardRow>) -> ModelRc<CardRow> {
    let weak = window.as_weak();
    let ctx = ctx.clone();
    ModelRc::new(CardGrid::new(move |request| {
        spawn_grid(weak.clone(), &ctx, binding, request);
    }))
}

fn grid_paging(page: usize) -> Paging {
    Paging {
        start_index: page * GRID_PAGE,
        limit: GRID_PAGE,
    }
}

fn spawn_grid(
    window: slint::Weak<AppWindow>,
    ctx: &Context,
    binding: Binding<CardRow>,
    request: GridRequest,
) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    match request {
        GridRequest::Page {
            generation,
            source,
            page,
        } => {
            ctx.rt.spawn(async move {
                let result = source.fetch(server.as_ref(), grid_paging(page)).await;
                on_ui(&window, move |window| {
                    apply_grid_page(window, binding, generation, page, result);
                });
            });
        }
        GridRequest::Cover { cover_id } => {
            let covers = ctx.covers.clone();
            ctx.rt.spawn(async move {
                let result = covers
                    .cover(server.as_ref(), &cover_id, GRID_COVER_SIZE)
                    .await;
                on_ui(&window, move |window| {
                    apply_grid_cover(window, binding, &cover_id, result);
                });
            });
        }
    }
}

fn apply_grid_page(
    window: &AppWindow,
    binding: Binding<CardRow>,
    generation: u64,
    page: usize,
    result: feishin_api::Result<CardPage>,
) {
    match result {
        Ok(CardPage { cards, total }) => {
            let applied = with_grid(window, binding, |grid| {
                grid.apply_page(generation, page, cards, total)
            });
            if applied == Some(true) {
                (binding.loading)(window, false);
            }
        }
        Err(error) => {
            let failed = with_grid(window, binding, |grid| grid.page_failed(generation, page));
            if failed == Some(true) {
                (binding.loading)(window, false);
                show_status(window, &format!("Could not load the list: {error}"));
            }
        }
    }
}

fn apply_grid_cover(
    window: &AppWindow,
    binding: Binding<CardRow>,
    cover_id: &str,
    result: anyhow::Result<Arc<DecodedImage>>,
) {
    let image = result
        .inspect_err(|error| tracing::debug!("grid cover {cover_id}: {error:#}"))
        .ok()
        .and_then(|cover| to_image(&cover));
    with_grid(window, binding, |grid| grid.apply_cover(cover_id, image));
}
