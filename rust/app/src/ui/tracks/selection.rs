use feishin_core::model::Song;

use super::listed::ListedSongs;
use super::selection_ranges::SelectionRanges;

#[derive(Debug, Default, Clone)]
pub struct Selection {
    ranges: SelectionRanges,
    anchor: Option<usize>,
}

impl Selection {
    pub fn contains(&self, index: usize) -> bool {
        self.ranges.contains(index)
    }

    pub fn len(&self) -> usize {
        self.ranges.row_count()
    }

    pub fn ranges(&self) -> &SelectionRanges {
        &self.ranges
    }

    pub fn click(&mut self, index: usize, toggle: bool, extend: bool) -> SelectionRanges {
        self.changing(|selection| match selection.anchor.filter(|_| extend) {
            Some(anchor) => selection.extend_from_anchor(anchor, index, toggle),
            None if toggle => selection.toggle_row(index),
            None => selection.select_only(index),
        })
    }

    pub fn focus(&mut self, index: usize) -> SelectionRanges {
        if self.contains(index) {
            return SelectionRanges::default();
        }
        self.click(index, false, false)
    }

    pub fn select_all(&mut self, row_count: usize) -> SelectionRanges {
        self.changing(|selection| {
            selection.ranges = SelectionRanges::spanning(0..row_count);
        })
    }

    pub fn clear(&mut self) -> SelectionRanges {
        self.changing(|selection| *selection = Self::default())
    }

    fn changing(&mut self, change: impl FnOnce(&mut Self)) -> SelectionRanges {
        let before = self.ranges.clone();
        change(self);
        before.symmetric_difference(&self.ranges)
    }

    fn extend_from_anchor(&mut self, anchor: usize, index: usize, keep_existing: bool) {
        if !keep_existing {
            self.ranges = SelectionRanges::default();
        }
        self.ranges
            .insert(anchor.min(index)..anchor.max(index).saturating_add(1));
    }

    fn toggle_row(&mut self, index: usize) {
        self.ranges.toggle(index);
        self.anchor = Some(index);
    }

    fn select_only(&mut self, index: usize) {
        self.ranges = SelectionRanges::spanning(index..index.saturating_add(1));
        self.anchor = Some(index);
    }
}

#[derive(Debug, Clone)]
pub enum SelectedSongs {
    Loaded(Vec<Song>),
    Listed(ListedSongs),
}

#[derive(Debug, Clone)]
pub struct FirstSelected {
    pub listed: ListedSongs,
    pub song: Option<Song>,
}
