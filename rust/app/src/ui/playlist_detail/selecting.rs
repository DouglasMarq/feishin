use std::sync::Arc;

use slint::{ComponentHandle, Model};

use super::detail::{SharedDetail, lock};
use super::selection::{ClickMode, Selection};
use crate::ui::{AppWindow, PlaylistDetailState, clamp_i32};

fn sync_selection(window: &AppWindow, before: &Selection, after: &Selection) {
    let state = window.global::<PlaylistDetailState>();
    let rows = state.get_rows();
    for index in after.changed_since(before) {
        if let Some(mut row) = rows.row_data(index) {
            row.selected = after.contains(index);
            rows.set_row_data(index, row);
        }
    }
    state.set_selected_count(clamp_i32(after.len() as u64));
}

fn update_selection(
    window: &AppWindow,
    shared: &SharedDetail,
    change: impl FnOnce(&mut Selection, usize),
) {
    let (before, after) = {
        let mut detail = lock(shared);
        let before = detail.selection.clone();
        let song_count = detail.songs.len();
        change(&mut detail.selection, song_count);
        (before, detail.selection.clone())
    };
    sync_selection(window, &before, &after);
}

pub fn install_selection(window: &AppWindow, shared: &SharedDetail) {
    let state = window.global::<PlaylistDetailState>();

    let (weak, select_shared) = (window.as_weak(), Arc::clone(shared));
    state.on_select(move |index, toggle_held, range_held| {
        let (Some(window), Ok(index)) = (weak.upgrade(), usize::try_from(index)) else {
            return;
        };
        let mode = ClickMode::from_modifiers(toggle_held, range_held);
        update_selection(&window, &select_shared, |selection, song_count| {
            if index < song_count {
                selection.click(index, mode);
            }
        });
    });

    let (weak, all_shared) = (window.as_weak(), Arc::clone(shared));
    state.on_select_all(move || {
        if let Some(window) = weak.upgrade() {
            update_selection(&window, &all_shared, Selection::select_all);
        }
    });

    let (weak, clear_shared) = (window.as_weak(), Arc::clone(shared));
    state.on_clear_selection(move || {
        if let Some(window) = weak.upgrade() {
            update_selection(&window, &clear_shared, |selection, _| selection.clear());
        }
    });
}
