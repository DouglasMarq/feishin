use slint::ComponentHandle;

use super::binding::with_model;
use super::model::PlaylistSongsModel;
use super::selection::ClickMode;
use crate::ui::{AppWindow, PlaylistDetailState, clamp_i32};

fn update_selection(window: &AppWindow, change: impl FnOnce(&PlaylistSongsModel) -> usize) {
    if let Some(count) = with_model(window, change) {
        window
            .global::<PlaylistDetailState>()
            .set_selected_count(clamp_i32(count as u64));
    }
}

pub fn install_selection(window: &AppWindow) {
    let state = window.global::<PlaylistDetailState>();

    let weak = window.as_weak();
    state.on_select(move |index, toggle_held, range_held| {
        let (Some(window), Ok(index)) = (weak.upgrade(), usize::try_from(index)) else {
            return;
        };
        let mode = ClickMode::from_modifiers(toggle_held, range_held);
        update_selection(&window, |model| model.click(index, mode));
    });

    let weak = window.as_weak();
    state.on_select_all(move || {
        if let Some(window) = weak.upgrade() {
            update_selection(&window, PlaylistSongsModel::select_all);
        }
    });

    let weak = window.as_weak();
    state.on_clear_selection(move || {
        if let Some(window) = weak.upgrade() {
            update_selection(&window, PlaylistSongsModel::clear_selection);
        }
    });
}
