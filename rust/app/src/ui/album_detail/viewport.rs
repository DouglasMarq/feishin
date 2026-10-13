use slint::ComponentHandle;

use super::super::{AlbumDetailState, AppWindow, clamp_i32};
use super::tracks::with_tracks;

fn as_slint_int(count: usize) -> i32 {
    clamp_i32(u64::try_from(count).unwrap_or(u64::MAX))
}

pub fn visible_slots(window: &AppWindow, viewport_height: f32) -> i32 {
    with_tracks(window, |tracks| tracks.visible_slots(viewport_height))
        .map(as_slint_int)
        .unwrap_or_default()
}

pub fn first_visible_row(window: &AppWindow, viewport_top: f32, slots: i32) -> i32 {
    let slots = usize::try_from(slots).unwrap_or_default();
    with_tracks(window, |tracks| {
        tracks.first_visible_row(viewport_top, slots)
    })
    .map(as_slint_int)
    .unwrap_or_default()
}

pub fn install_viewport(window: &AppWindow) {
    let state = window.global::<AlbumDetailState>();
    let weak_window = window.as_weak();
    state.on_visible_slots(move |viewport_height, _row_count| {
        weak_window
            .upgrade()
            .map(|window| visible_slots(&window, viewport_height))
            .unwrap_or_default()
    });

    let weak_window = window.as_weak();
    state.on_first_visible_row(move |viewport_top, slots| {
        weak_window
            .upgrade()
            .map(|window| first_visible_row(&window, viewport_top, slots))
            .unwrap_or_default()
    });
}
