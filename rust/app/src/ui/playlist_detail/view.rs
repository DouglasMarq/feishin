use std::sync::Arc;

use slint::ComponentHandle;

use super::actions::{SongAction, act_on_rows};
use super::binding::{install_model, with_model};
use super::detail::SharedDetail;
use super::editing::remove;
use super::loading::load;
use super::playing::{PlayStart, play_playlist};
use super::selecting::install_selection;
use crate::ui::{AppWindow, Context, PlaylistDetailState};

pub fn install(window: &AppWindow, ctx: &Context) {
    let shared: SharedDetail = Arc::default();
    install_model(window, ctx);
    install_load(window, ctx, &shared);
    install_play(window, ctx, &shared);
    install_play_row(window, ctx, &shared);
    install_selection(window);
    install_selection_actions(window, ctx, &shared);
    install_context_action(window, ctx, &shared);
}

fn install_load(window: &AppWindow, ctx: &Context, shared: &SharedDetail) {
    let (weak, load_ctx, load_shared) = (window.as_weak(), ctx.clone(), Arc::clone(shared));
    window
        .global::<PlaylistDetailState>()
        .on_load(move |playlist_id| {
            if let Some(window) = weak.upgrade() {
                load(&window, &load_ctx, &load_shared, &playlist_id);
            }
        });
}

fn install_play(window: &AppWindow, ctx: &Context, shared: &SharedDetail) {
    let (weak, play_ctx, play_shared) = (window.as_weak(), ctx.clone(), Arc::clone(shared));
    window
        .global::<PlaylistDetailState>()
        .on_play(move |shuffle| {
            let Some(window) = weak.upgrade() else {
                return;
            };
            if with_model(&window, |model| model.total()).unwrap_or_default() == 0 {
                return;
            }
            let start = PlayStart::FromTop { shuffle };
            play_playlist(&window, &play_ctx, &play_shared, start);
        });
}

fn install_play_row(window: &AppWindow, ctx: &Context, shared: &SharedDetail) {
    let (weak, row_ctx, row_shared) = (window.as_weak(), ctx.clone(), Arc::clone(shared));
    window
        .global::<PlaylistDetailState>()
        .on_play_row(move |index| {
            let (Some(window), Ok(index)) = (weak.upgrade(), usize::try_from(index)) else {
                return;
            };
            let Some(song_id) = with_model(&window, |model| model.song_id_at(index)).flatten()
            else {
                return;
            };
            play_playlist(&window, &row_ctx, &row_shared, PlayStart::AtSong(song_id));
        });
}

fn install_selection_actions(window: &AppWindow, ctx: &Context, shared: &SharedDetail) {
    let state = window.global::<PlaylistDetailState>();

    let (weak, remove_ctx, remove_shared) = (window.as_weak(), ctx.clone(), Arc::clone(shared));
    state.on_remove_selected(move || {
        if let Some(window) = weak.upgrade() {
            let indexes = with_model(&window, |model| model.selected_indexes()).unwrap_or_default();
            remove(&window, &remove_ctx, &remove_shared, indexes);
        }
    });

    let (weak, add_ctx) = (window.as_weak(), ctx.clone());
    state.on_add_selected_to_playlist(move || {
        if let Some(window) = weak.upgrade() {
            let indexes = with_model(&window, |model| model.selected_indexes()).unwrap_or_default();
            act_on_rows(&window, &add_ctx, &indexes, SongAction::AddToPlaylist);
        }
    });
}

fn install_context_action(window: &AppWindow, ctx: &Context, shared: &SharedDetail) {
    let (weak, menu_ctx, menu_shared) = (window.as_weak(), ctx.clone(), Arc::clone(shared));
    window
        .global::<PlaylistDetailState>()
        .on_context_action(move |index, action| {
            let (Some(window), Ok(index)) = (weak.upgrade(), usize::try_from(index)) else {
                return;
            };
            let targets = with_model(&window, |model| {
                (index < model.total()).then(|| model.targets(index))
            })
            .flatten();
            let Some(targets) = targets else {
                return;
            };
            match SongAction::from_menu(action) {
                Some(song_action) => act_on_rows(&window, &menu_ctx, &targets, song_action),
                None => remove(&window, &menu_ctx, &menu_shared, targets),
            }
        });
}
