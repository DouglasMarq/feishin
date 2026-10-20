use std::rc::Rc;

use slint::{ComponentHandle, ModelRc};

use super::editing::{create_playlist, delete_playlist};
use super::loading::{fetch_cover, load};
use super::model::PlaylistsModel;
use super::play_mode::{PlayMode, play};
use crate::ui::{AppWindow, Context, PlaylistsState};

pub fn install(window: &AppWindow, context: &Context) {
    let state = window.global::<PlaylistsState>();
    let weak = window.as_weak();
    let cover_context = context.clone();
    state.set_rows(ModelRc::new(Rc::new(PlaylistsModel::new(
        move |cover_id| {
            fetch_cover(weak.clone(), &cover_context, cover_id);
        },
    ))));
    install_load(window, context);
    install_play(window, context);
    install_create(window, context);
    install_delete(window, context);
}

fn install_load(window: &AppWindow, context: &Context) {
    let weak = window.as_weak();
    let load_context = context.clone();
    window.global::<PlaylistsState>().on_load(move |_| {
        if let Some(window) = weak.upgrade() {
            load(&window, &load_context);
        }
    });
}

fn install_play(window: &AppWindow, context: &Context) {
    let weak = window.as_weak();
    let play_context = context.clone();
    window.global::<PlaylistsState>().on_play(move |id, mode| {
        if let Some(window) = weak.upgrade() {
            play(&play_context, &window, &id, PlayMode::from(mode));
        }
    });
}

fn install_create(window: &AppWindow, context: &Context) {
    let weak = window.as_weak();
    let create_context = context.clone();
    window.global::<PlaylistsState>().on_create(move |name| {
        if let Some(window) = weak.upgrade() {
            create_playlist(&window, &create_context, &name);
        }
    });
}

fn install_delete(window: &AppWindow, context: &Context) {
    let weak = window.as_weak();
    let delete_context = context.clone();
    window.global::<PlaylistsState>().on_delete(move |id| {
        if let Some(window) = weak.upgrade() {
            delete_playlist(&window, &delete_context, id);
        }
    });
}
