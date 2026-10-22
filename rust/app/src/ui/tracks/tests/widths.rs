use slint::ComponentHandle;

use crate::ui::{AppWindow, TrackColumn, TracksState};

const ALL_COLUMNS: [TrackColumn; 9] = [
    TrackColumn::Index,
    TrackColumn::Title,
    TrackColumn::Artist,
    TrackColumn::Album,
    TrackColumn::Year,
    TrackColumn::Plays,
    TrackColumn::Duration,
    TrackColumn::Favorite,
    TrackColumn::Rating,
];
const TOLERANCE: f32 = 0.5;

fn open() -> AppWindow {
    i_slint_backend_testing::init_no_event_loop();
    AppWindow::new().unwrap()
}

fn row_width(window: &AppWindow, list_width: f32) -> f32 {
    let state = window.global::<TracksState>();
    ALL_COLUMNS
        .into_iter()
        .map(|column| state.invoke_shown_width(column, list_width))
        .sum()
}

#[test]
fn given_a_narrow_window_the_rating_column_stays_visible() {
    let window = open();
    let list_width = 900.0;
    assert!(row_width(&window, list_width) <= list_width);
}

#[test]
fn given_a_wide_window_the_text_columns_fill_it() {
    let window = open();
    let state = window.global::<TracksState>();
    let list_width = 2000.0;
    let trailing_gap = state.get_trailing_gap();
    assert!((row_width(&window, list_width) + trailing_gap - list_width).abs() < TOLERANCE);
    assert!(state.invoke_shown_width(TrackColumn::Title, list_width) > state.get_w_title());
    assert!(
        (state.invoke_shown_width(TrackColumn::Rating, list_width) - state.get_w_rating()).abs()
            < TOLERANCE
    );
}

#[test]
fn given_a_window_too_narrow_for_the_text_columns_they_keep_their_minimum() {
    let window = open();
    let state = window.global::<TracksState>();
    assert_eq!(
        state.invoke_shown_width(TrackColumn::Artist, 400.0),
        state.get_min_flexible_width()
    );
}

#[test]
fn given_a_dragged_title_edge_it_follows_the_pointer() {
    let window = open();
    let state = window.global::<TracksState>();
    let list_width = 1200.0;
    let dragged_width = state.invoke_shown_width(TrackColumn::Title, list_width) + 50.0;
    state.invoke_resize(TrackColumn::Title, dragged_width, list_width);
    assert!(
        (state.invoke_shown_width(TrackColumn::Title, list_width) - dragged_width).abs()
            < TOLERANCE
    );
}

#[test]
fn given_a_dragged_fixed_column_its_width_is_saved_as_is() {
    let window = open();
    let state = window.global::<TracksState>();
    state.invoke_resize(TrackColumn::Rating, 140.0, 1200.0);
    assert_eq!(state.get_w_rating(), 140.0);
}
