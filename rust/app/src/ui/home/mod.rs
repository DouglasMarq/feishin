mod model;
mod requests;
mod shelf;
mod shown_covers;

#[cfg(test)]
mod headless_tests;
#[cfg(test)]
mod test_albums;

use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use slint::{ComponentHandle, ModelRc};

use super::{AppWindow, Context, HomeState};
use model::{HomeModel, with_home_model};
use requests::{load_cover, load_shelf, play_album};

fn install_load(window: &AppWindow, ctx: &Context) {
    let weak_window = window.as_weak();
    let ctx = ctx.clone();
    window.global::<HomeState>().on_load(move |_| {
        let (Some(window), Some(server)) = (weak_window.upgrade(), ctx.session.get()) else {
            return;
        };
        let shelves = with_home_model(&window, |model| model.visit(&server.config().id));
        for shelf in shelves.unwrap_or_default() {
            load_shelf(&window, &ctx, shelf);
        }
    });
}

fn install_refresh(window: &AppWindow, ctx: &Context) {
    let weak_window = window.as_weak();
    let ctx = ctx.clone();
    window.global::<HomeState>().on_refresh(move |shelf| {
        if let (Some(window), Ok(shelf)) = (weak_window.upgrade(), usize::try_from(shelf)) {
            load_shelf(&window, &ctx, shelf);
        }
    });
}

fn install_cover_needed(window: &AppWindow, ctx: &Context) {
    let weak_window = window.as_weak();
    let ctx = ctx.clone();
    window
        .global::<HomeState>()
        .on_cover_needed(move |shelf, index| {
            if let (Some(window), Ok(shelf), Ok(index)) = (
                weak_window.upgrade(),
                usize::try_from(shelf),
                usize::try_from(index),
            ) {
                load_cover(&window, &ctx, shelf, index);
            }
        });
}

fn install_play(window: &AppWindow, ctx: &Context) {
    let weak_window = window.as_weak();
    let ctx = ctx.clone();
    let latest_request = Arc::new(AtomicU64::new(0));
    window.global::<HomeState>().on_play(move |album_id| {
        if let Some(window) = weak_window.upgrade() {
            play_album(&window, &ctx, &latest_request, album_id.to_string());
        }
    });
}

pub fn install(window: &AppWindow, ctx: &Context) {
    window
        .global::<HomeState>()
        .set_shelves(ModelRc::new(HomeModel::default()));
    install_load(window, ctx);
    install_refresh(window, ctx);
    install_cover_needed(window, ctx);
    install_play(window, ctx);
}
