use feishin_core::model::Song;

use super::{LOADED_SELECTION_LIMIT, ListState, TracksModel};
use crate::list_stream::SongSource;
use crate::ui::tracks::listed::ListedSongs;
use crate::ui::tracks::selection::{FirstSelected, SelectedSongs, Selection};
use crate::ui::tracks::selection_ranges::SelectionRanges;

impl ListState {
    fn loaded_selection(&self) -> Option<Vec<Song>> {
        let ranges = self.selection.ranges();
        if ranges.row_count() > LOADED_SELECTION_LIMIT {
            return None;
        }
        ranges.rows().map(|row| self.song(row).cloned()).collect()
    }

    fn listed_selection(&self) -> ListedSongs {
        ListedSongs {
            source: SongSource::Library(self.query.clone()),
            ranges: self.selection.ranges().clone(),
        }
    }
}

impl TracksModel {
    pub fn select(&self, index: usize, toggle: bool, extend: bool) -> usize {
        self.change_selection(Some(index), |selection, _| {
            selection.click(index, toggle, extend)
        })
    }

    pub fn focus(&self, index: usize) -> usize {
        self.change_selection(Some(index), |selection, _| selection.focus(index))
    }

    pub fn select_all(&self) -> usize {
        self.change_selection(None, Selection::select_all)
    }

    pub fn clear_selection(&self) -> usize {
        self.change_selection(None, |selection, _| selection.clear())
    }

    fn change_selection(
        &self,
        index: Option<usize>,
        change: impl FnOnce(&mut Selection, usize) -> SelectionRanges,
    ) -> usize {
        let (changed, count) = {
            let mut state = self.state.borrow_mut();
            let total = state.total;
            if index.is_some_and(|index| index >= total) {
                return state.selection.len();
            }
            let changed = change(&mut state.selection, total);
            (changed, state.selection.len())
        };
        self.notify_rows(changed.rows());
        count
    }

    pub fn selected_songs(&self) -> SelectedSongs {
        let state = self.state.borrow();
        state.loaded_selection().map_or_else(
            || SelectedSongs::Listed(state.listed_selection()),
            SelectedSongs::Loaded,
        )
    }

    pub fn first_selected(&self) -> Option<FirstSelected> {
        let state = self.state.borrow();
        let row = state.selection.ranges().first_row()?;
        Some(FirstSelected {
            listed: ListedSongs {
                source: SongSource::Library(state.query.clone()),
                ranges: SelectionRanges::spanning(row..row.saturating_add(1)),
            },
            song: state.song(row).cloned(),
        })
    }
}
