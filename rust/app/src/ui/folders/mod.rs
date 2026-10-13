mod actions;
mod binding;
mod loading;
mod model;
mod trail;

#[cfg(test)]
mod model_tests;
#[cfg(test)]
mod view_tests;

use slint::ComponentHandle;

use crate::ui::{AppState, AppWindow, Context, FoldersState, Route};
use trail::{Crumb, SharedTrail};

pub fn install(window: &AppWindow, ctx: &Context) {
    let trail = SharedTrail::default();
    binding::install_model(window, ctx);
    install_load(window, ctx, &trail);
    install_open(window, &trail);
    install_playing(window, ctx);
}

fn install_load(window: &AppWindow, ctx: &Context, trail: &SharedTrail) {
    let (weak, load_ctx, load_trail) = (window.as_weak(), ctx.clone(), trail.clone());
    window.global::<FoldersState>().on_load(move |detail_id| {
        if let Some(window) = weak.upgrade() {
            loading::load(&window, &load_ctx, &load_trail, &detail_id);
        }
    });
}

fn install_open(window: &AppWindow, trail: &SharedTrail) {
    let (weak, open_trail) = (window.as_weak(), trail.clone());
    window
        .global::<FoldersState>()
        .on_open(move |folder_id, name| {
            let Some(window) = weak.upgrade() else {
                return;
            };
            let parent_id = window.global::<AppState>().get_detail_id();
            open_trail
                .borrow_mut()
                .enter(&parent_id, Crumb::new(&folder_id, &name));
            window
                .global::<AppState>()
                .invoke_navigate(Route::Folders, folder_id);
        });
}

fn install_playing(window: &AppWindow, ctx: &Context) {
    let state = window.global::<FoldersState>();

    let (weak, folder_ctx) = (window.as_weak(), ctx.clone());
    state.on_play_folder(move |folder_id, mode| {
        if let Some(window) = weak.upgrade() {
            actions::play_folder(&window, &folder_ctx, &folder_id, mode);
        }
    });

    let (weak, row_ctx) = (window.as_weak(), ctx.clone());
    state.on_play_row(move |row| {
        if let (Some(window), Ok(row)) = (weak.upgrade(), usize::try_from(row)) {
            actions::play_row(&window, &row_ctx, row);
        }
    });

    let (weak, song_ctx) = (window.as_weak(), ctx.clone());
    state.on_song_action(move |row, action| {
        if let (Some(window), Ok(row)) = (weak.upgrade(), usize::try_from(row)) {
            actions::act_on_song(&window, &song_ctx, row, action);
        }
    });
}
