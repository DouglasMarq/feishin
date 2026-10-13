use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_core::model::{AlbumListQuery, Paging};
use slint::{Model, ModelRc, Weak};

use super::super::player::to_image;
use super::super::{AlbumGridRow, AppWindow, Context, on_ui, show_status};
use super::card::CardData;
use super::model::{AlbumGridModel, GridRequest, PAGE_SIZE};
use crate::covers::CoverProvider;

pub const COVER_SIZE: u32 = 256;

pub type Locate = fn(&AppWindow) -> ModelRc<AlbumGridRow>;

pub type OnCount = fn(&AppWindow, anyhow::Result<usize>);

pub fn with_model<Output>(
    window: &AppWindow,
    locate: Locate,
    action: impl FnOnce(&AlbumGridModel) -> Output,
) -> Option<Output> {
    let rows = locate(window);
    rows.as_any().downcast_ref::<AlbumGridModel>().map(action)
}

struct GridTarget {
    window: Weak<AppWindow>,
    locate: Locate,
}

async fn fetch_count(
    server: Arc<dyn MusicServer>,
    target: GridTarget,
    on_count: OnCount,
    generation: u64,
    query: AlbumListQuery,
) {
    let result = server.album_count(&query).await;
    let locate = target.locate;
    on_ui(&target.window, move |window| {
        let total = result.as_ref().map_or(0, |total| *total);
        let is_current_count =
            with_model(window, locate, |model| model.apply_count(generation, total));
        if is_current_count == Some(true) {
            on_count(window, result.map_err(Into::into));
        }
    });
}

async fn fetch_page(
    server: Arc<dyn MusicServer>,
    target: GridTarget,
    generation: u64,
    query: AlbumListQuery,
    page: usize,
) {
    let paging = Paging {
        start_index: page * PAGE_SIZE,
        limit: PAGE_SIZE,
    };
    let result = server
        .album_list(&query, paging)
        .await
        .map(|albums| albums.items.iter().map(CardData::from_album).collect());
    let locate = target.locate;
    on_ui(&target.window, move |window| match result {
        Ok(cards) => {
            with_model(window, locate, |model| {
                model.apply_page(generation, page, cards)
            });
        }
        Err(error) => {
            with_model(window, locate, |model| model.page_failed(generation, page));
            show_status(window, &format!("Could not load albums: {error}"));
        }
    });
}

async fn fetch_cover(
    server: Arc<dyn MusicServer>,
    covers: Arc<dyn CoverProvider>,
    target: GridTarget,
    cover_art_id: String,
) {
    let result = covers
        .cover(server.as_ref(), &cover_art_id, COVER_SIZE)
        .await;
    if let Err(error) = &result {
        tracing::debug!("cover {cover_art_id}: {error:#}");
    }
    let locate = target.locate;
    on_ui(&target.window, move |window| {
        let image = result.ok().and_then(|cover| to_image(&cover));
        with_model(window, locate, |model| {
            model.apply_cover(&cover_art_id, image)
        });
    });
}

pub fn spawn_request(
    window: Weak<AppWindow>,
    ctx: &Context,
    locate: Locate,
    on_count: OnCount,
    request: GridRequest,
) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let covers = ctx.covers.clone();
    let target = GridTarget { window, locate };
    ctx.rt.spawn(async move {
        match request {
            GridRequest::Count { generation, query } => {
                fetch_count(server, target, on_count, generation, query).await;
            }
            GridRequest::Page {
                generation,
                query,
                page,
            } => fetch_page(server, target, generation, query, page).await,
            GridRequest::Cover { cover_art_id } => {
                fetch_cover(server, covers, target, cover_art_id).await;
            }
        }
    });
}
