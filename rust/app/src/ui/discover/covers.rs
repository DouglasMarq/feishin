use std::sync::Arc;

use feishin_storage::cover_cache::DecodedImage;
use slint::{ComponentHandle, Image};

use super::cards::cover_key;
use super::deps::DiscoverDeps;
use super::model::with_discover;
use crate::ui::images::to_image;
use crate::ui::{AppWindow, DiscoverState, on_ui};

const CARD_COVER_SIZE: u32 = 192;
const SONG_COVER_SIZE: u32 = 64;
const PAGE_COVER_SIZE: u32 = 300;

fn fetch_cover(
    window: &AppWindow,
    deps: &DiscoverDeps,
    url: String,
    size: u32,
    done: impl FnOnce(&AppWindow, Option<Image>) + Send + 'static,
) {
    let covers = Arc::clone(&deps.ctx.covers);
    let weak_window = window.as_weak();
    deps.ctx.rt.spawn(async move {
        let decoded: Option<Arc<DecodedImage>> = covers
            .cover_from_url(&cover_key(&url), &url, size)
            .await
            .inspect_err(|error| tracing::debug!("YouTube Music cover {url}: {error:#}"))
            .ok();
        on_ui(&weak_window, move |window| {
            done(window, decoded.and_then(|cover| to_image(&cover)));
        });
    });
}

pub fn load_card_cover(window: &AppWindow, deps: &DiscoverDeps, shelf: usize, card: usize) {
    let claimed = with_discover(window, |model| model.request_card_cover(shelf, card)).flatten();
    let Some((request, card_id)) = claimed else {
        return;
    };
    let generation = request.generation;
    fetch_cover(
        window,
        deps,
        request.url,
        CARD_COVER_SIZE,
        move |window, cover| {
            with_discover(window, |model| {
                model.set_card_cover(generation, shelf, &card_id, cover);
            });
        },
    );
}

pub fn load_song_cover(window: &AppWindow, deps: &DiscoverDeps, index: usize) {
    let Some(request) = with_discover(window, |model| model.request_song_cover(index)).flatten()
    else {
        return;
    };
    let generation = request.generation;
    fetch_cover(
        window,
        deps,
        request.url,
        SONG_COVER_SIZE,
        move |window, cover| {
            with_discover(window, |model| {
                model.set_song_cover(generation, index, cover)
            });
        },
    );
}

pub fn load_page_cover(window: &AppWindow, deps: &DiscoverDeps, generation: u64, url: String) {
    fetch_cover(window, deps, url, PAGE_COVER_SIZE, move |window, cover| {
        let current = with_discover(window, |model| model.page.borrow().generation == generation);
        let (Some(true), Some(cover)) = (current, cover) else {
            return;
        };
        let state = window.global::<DiscoverState>();
        state.set_page_cover(cover);
        state.set_page_has_cover(true);
    });
}

pub fn card_visibility_changed(window: &AppWindow, shelf: usize, card: usize, visible: bool) {
    with_discover(window, |model| model.set_card_visible(shelf, card, visible));
}

pub fn song_rows_shown(window: &AppWindow, first_row: usize, row_count: usize) {
    with_discover(window, |model| {
        model.show_song_rows(first_row..first_row.saturating_add(row_count));
    });
}
