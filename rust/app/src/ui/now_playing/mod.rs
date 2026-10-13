use std::rc::Rc;

use slint::{ComponentHandle, ModelRc};

use super::{AppWindow, Context, NowPlayingState, queue};
use crate::controller::Command;

mod fetching;
mod loading;
mod lyrics_cache;
mod lyrics_plan;
mod song_lyrics;
mod up_next;
mod view;

#[cfg(test)]
mod view_tests;

pub use up_next::UpNextModel;
use view::{View, register_view};

fn install_view_callbacks(
    state: &NowPlayingState,
    window: &AppWindow,
    ctx: &Context,
    view: &Rc<View>,
) {
    let weak_window = window.as_weak();
    let load_ctx = ctx.clone();
    let load_view = Rc::clone(view);
    state.on_load(move |song_id| {
        if let Some(window) = weak_window.upgrade() {
            loading::load(&window, &load_ctx, &load_view, &song_id);
        }
    });

    let weak_window = window.as_weak();
    let position_view = Rc::clone(view);
    state.on_position_changed(move || {
        if let Some(window) = weak_window.upgrade() {
            position_view.highlight_active_line(&window);
        }
    });

    let weak_window = window.as_weak();
    let up_next_view = Rc::clone(view);
    state.on_refresh_up_next(move || {
        if let Some(window) = weak_window.upgrade() {
            up_next_view.refresh_up_next(&window);
        }
    });
}

fn install_play_similar(state: &NowPlayingState, ctx: &Context, view: Rc<View>) {
    let play_ctx = ctx.clone();
    state.on_play_similar(move |song_id| {
        let songs = view.similar.borrow().clone();
        if !songs.is_empty() {
            play_ctx.send(Command::PlaySongs {
                songs,
                song_id: Some(song_id.to_string()),
            });
        }
    });
}

pub fn install(window: &AppWindow, ctx: &Context) {
    let state = window.global::<NowPlayingState>();
    let up_next = Rc::new(UpNextModel::new(queue::shared_view(window)));
    let view = Rc::new(View::new(Rc::clone(&up_next)));
    register_view(&view);
    state.set_up_next(ModelRc::from(up_next));
    install_view_callbacks(&state, window, ctx, &view);
    install_play_similar(&state, ctx, view);
}
