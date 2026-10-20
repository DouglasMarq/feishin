use std::sync::Arc;

use slint::ComponentHandle;

use super::actions::{SongAction, act_on_rows};
use super::binding::{install_model, with_model};
use super::detail::SharedDetail;
use super::editing::remove;
use super::loading::load;
use super::playing::{PlayStart, play_playlist};
use super::selecting::install_selection;
use crate::ui::playlist_picker::PlaylistPicker;
use crate::ui::{AppWindow, Context, PlaylistDetailState};

pub fn install(window: &AppWindow, context: &Context, picker: &PlaylistPicker) {
    let shared: SharedDetail = Arc::default();
    install_model(window, context);
    install_load(window, context, &shared);
    install_play(window, context, &shared);
    install_play_row(window, context, &shared);
    install_selection(window);
    install_selection_actions(window, context, &shared, picker);
    install_context_action(window, context, &shared, picker);
}

fn install_load(window: &AppWindow, context: &Context, shared: &SharedDetail) {
    let (weak, load_context, load_shared) = (window.as_weak(), context.clone(), Arc::clone(shared));
    window
        .global::<PlaylistDetailState>()
        .on_load(move |playlist_id| {
            if let Some(window) = weak.upgrade() {
                load(&window, &load_context, &load_shared, &playlist_id);
            }
        });
}

fn install_play(window: &AppWindow, context: &Context, shared: &SharedDetail) {
    let (weak, play_context, play_shared) = (window.as_weak(), context.clone(), Arc::clone(shared));
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
            play_playlist(&window, &play_context, &play_shared, start);
        });
}

fn install_play_row(window: &AppWindow, context: &Context, shared: &SharedDetail) {
    let (weak, row_context, row_shared) = (window.as_weak(), context.clone(), Arc::clone(shared));
    window
        .global::<PlaylistDetailState>()
        .on_play_row(move |index| {
            let (Some(window), Ok(index)) = (weak.upgrade(), usize::try_from(index)) else {
                return;
            };
            let Some(song_id) = with_model(&window, |model| {
                (index < model.total()).then(|| model.song_id_at(index))
            })
            .flatten() else {
                return;
            };
            let start = PlayStart::AtRow { index, song_id };
            play_playlist(&window, &row_context, &row_shared, start);
        });
}

fn install_selection_actions(
    window: &AppWindow,
    context: &Context,
    shared: &SharedDetail,
    picker: &PlaylistPicker,
) {
    let state = window.global::<PlaylistDetailState>();

    let (weak, remove_context, remove_shared) =
        (window.as_weak(), context.clone(), Arc::clone(shared));
    state.on_remove_selected(move || {
        if let Some(window) = weak.upgrade() {
            let indexes = with_model(&window, |model| model.selected_indexes()).unwrap_or_default();
            remove(&window, &remove_context, &remove_shared, indexes);
        }
    });

    let (weak, add_context, add_picker) = (window.as_weak(), context.clone(), picker.clone());
    state.on_add_selected_to_playlist(move || {
        if let Some(window) = weak.upgrade() {
            let indexes = with_model(&window, |model| model.selected_indexes()).unwrap_or_default();
            let action = SongAction::AddToPlaylist;
            act_on_rows(&window, &add_context, &add_picker, &indexes, action);
        }
    });
}

fn install_context_action(
    window: &AppWindow,
    context: &Context,
    shared: &SharedDetail,
    picker: &PlaylistPicker,
) {
    let (weak, menu_context, menu_shared) = (window.as_weak(), context.clone(), Arc::clone(shared));
    let menu_picker = picker.clone();
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
                Some(song_action) => {
                    act_on_rows(&window, &menu_context, &menu_picker, &targets, song_action);
                }
                None => remove(&window, &menu_context, &menu_shared, targets),
            }
        });
}
