use std::collections::VecDeque;

use slint::SharedString;

pub type ShownCover = (usize, SharedString);

#[derive(Default)]
pub struct ShownCovers {
    oldest_first: VecDeque<ShownCover>,
}

impl ShownCovers {
    pub fn record(
        &mut self,
        shelf: usize,
        album_id: SharedString,
        capacity: usize,
    ) -> Vec<ShownCover> {
        self.oldest_first
            .retain(|(shown_shelf, shown_id)| (*shown_shelf, shown_id) != (shelf, &album_id));
        self.oldest_first.push_back((shelf, album_id));
        let excess = self.oldest_first.len().saturating_sub(capacity);
        self.oldest_first.drain(..excess).collect()
    }

    pub fn forget_shelf(&mut self, shelf: usize) {
        self.oldest_first
            .retain(|(shown_shelf, _)| *shown_shelf != shelf);
    }

    pub fn clear(&mut self) {
        self.oldest_first.clear();
    }
}
