use std::rc::Rc;
use std::sync::Arc;

use feishin_api::{ApiError, MusicServer};
use feishin_core::model::{ArtistListQuery, Paging};
use slint::{ComponentHandle, Model, Weak};

use super::super::player::to_image;
use super::super::{AppWindow, ArtistsState, Context, on_ui, show_status};
use super::item::ArtistItem;
use super::list::{ArtistList, PAGE_SIZE, Request};
use super::models::ListModel;
use super::query::build_query;
use crate::covers::CoverProvider;

const COVER_SIZE: u32 = 200;

fn shared_list(window: &AppWindow) -> Option<Rc<ArtistList>> {
    window
        .global::<ArtistsState>()
        .get_items()
        .as_any()
        .downcast_ref::<ListModel>()
        .map(|model| Rc::clone(&model.list))
}

pub fn with_list<Output>(
    window: &AppWindow,
    action: impl FnOnce(&ArtistList) -> Output,
) -> Option<Output> {
    shared_list(window).map(|list| action(&list))
}

fn finish_loading(window: &AppWindow, list: &ArtistList) {
    let state = window.global::<ArtistsState>();
    state.set_loading(false);
    state.set_complete(list.is_complete());
}

fn apply_page_result(
    window: &AppWindow,
    generation: u64,
    start: usize,
    result: Result<Vec<ArtistItem>, ApiError>,
) {
    let Some(list) = shared_list(window) else {
        return;
    };
    match result {
        Ok(items) => {
            if list.apply_page(generation, start, items) {
                finish_loading(window, &list);
            }
        }
        Err(error) => {
            if list.page_failed(generation) {
                finish_loading(window, &list);
                show_status(window, &format!("Could not load artists: {error}"));
            }
        }
    }
}

async fn fetch_page(
    server: Arc<dyn MusicServer>,
    weak_window: Weak<AppWindow>,
    generation: u64,
    query: ArtistListQuery,
    start: usize,
) {
    let paging = Paging {
        start_index: start,
        limit: PAGE_SIZE,
    };
    let result = server
        .artist_list(&query, paging)
        .await
        .map(|page| page.items.into_iter().map(ArtistItem::from).collect());
    on_ui(&weak_window, move |window| {
        apply_page_result(window, generation, start, result);
    });
}

async fn fetch_cover(
    server: Arc<dyn MusicServer>,
    covers: Arc<dyn CoverProvider>,
    weak_window: Weak<AppWindow>,
    generation: u64,
    index: usize,
    cover_art_id: String,
) {
    let cover = covers
        .cover(server.as_ref(), &cover_art_id, COVER_SIZE)
        .await;
    if let Err(error) = &cover {
        tracing::debug!("artist cover {cover_art_id}: {error:#}");
    }
    on_ui(&weak_window, move |window| {
        let image = cover.ok().and_then(|cover| to_image(&cover));
        with_list(window, |list| list.apply_cover(generation, index, image));
    });
}

pub fn spawn_request(weak_window: Weak<AppWindow>, ctx: &Context, request: Request) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let covers = ctx.covers.clone();
    ctx.rt.spawn(async move {
        match request {
            Request::Page {
                generation,
                query,
                start,
            } => fetch_page(server, weak_window, generation, query, start).await,
            Request::Cover {
                generation,
                index,
                cover_art_id,
            } => {
                fetch_cover(server, covers, weak_window, generation, index, cover_art_id).await;
            }
        }
    });
}

pub fn reload(window: &AppWindow, search: &str) {
    let state = window.global::<ArtistsState>();
    let query = build_query(
        state.get_sort_index(),
        state.get_descending(),
        state.get_favorites_only(),
        search,
    );
    state.set_loading(true);
    state.set_complete(false);
    with_list(window, |list| list.reset(query));
}
