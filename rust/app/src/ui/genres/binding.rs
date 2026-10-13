use slint::{Model, ModelRc};

use super::card_grid::CardGrid;
use super::track_table::TrackTable;
use crate::ui::{AppWindow, CardRow, TrackItem};

pub struct Binding<Row: 'static> {
    pub rows: fn(&AppWindow) -> ModelRc<Row>,
    pub loading: fn(&AppWindow, bool),
}

impl<Row> Clone for Binding<Row> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Row> Copy for Binding<Row> {}

pub fn with_table<Output>(
    window: &AppWindow,
    binding: Binding<TrackItem>,
    action: impl FnOnce(&TrackTable) -> Output,
) -> Option<Output> {
    let rows = (binding.rows)(window);
    rows.as_any().downcast_ref::<TrackTable>().map(action)
}

pub fn with_grid<Output>(
    window: &AppWindow,
    binding: Binding<CardRow>,
    action: impl FnOnce(&CardGrid) -> Output,
) -> Option<Output> {
    let rows = (binding.rows)(window);
    rows.as_any().downcast_ref::<CardGrid>().map(action)
}

pub fn set_grid_columns(window: &AppWindow, binding: Binding<CardRow>, columns: i32) {
    with_grid(window, binding, |grid| {
        grid.set_columns(usize::try_from(columns).unwrap_or(1));
    });
}
