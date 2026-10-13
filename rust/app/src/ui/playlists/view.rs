use std::rc::Rc;

use slint::{ComponentHandle, ModelRc};

use super::editing::{create_playlist, delete_playlist};
use super::loading::{fetch_cover, load};
use super::model::PlaylistsModel;
use super::play_mode::{PlayMode, play};
use crate::ui::{AppWindow, Context, PlaylistsState};

pub fn install(window: &AppWindow, ctx: &Context) {
    let state = window.global::<PlaylistsState>();
    let weak = window.as_weak();
    let cover_ctx = ctx.clone();
    state.set_rows(ModelRc::new(Rc::new(PlaylistsModel::new(
        move |cover_id| {
            fetch_cover(weak.clone(), &cover_ctx, cover_id);
        },
    ))));
    install_load(window, ctx);
    install_play(window, ctx);
    install_create(window, ctx);
    install_delete(window, ctx);
}

fn install_load(window: &AppWindow, ctx: &Context) {
    let weak = window.as_weak();
    let load_ctx = ctx.clone();
    window.global::<PlaylistsState>().on_load(move |_| {
        if let Some(window) = weak.upgrade() {
            load(&window, &load_ctx);
        }
    });
}

fn install_play(window: &AppWindow, ctx: &Context) {
    let weak = window.as_weak();
    let play_ctx = ctx.clone();
    window.global::<PlaylistsState>().on_play(move |id, mode| {
        if let Some(window) = weak.upgrade() {
            play(&play_ctx, &window, &id, PlayMode::from(mode));
        }
    });
}

fn install_create(window: &AppWindow, ctx: &Context) {
    let weak = window.as_weak();
    let create_ctx = ctx.clone();
    window.global::<PlaylistsState>().on_create(move |name| {
        if let Some(window) = weak.upgrade() {
            create_playlist(&window, &create_ctx, &name);
        }
    });
}

fn install_delete(window: &AppWindow, ctx: &Context) {
    let weak = window.as_weak();
    let delete_ctx = ctx.clone();
    window.global::<PlaylistsState>().on_delete(move |id| {
        if let Some(window) = weak.upgrade() {
            delete_playlist(&window, &delete_ctx, id);
        }
    });
}
