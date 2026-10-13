use std::iter;

use super::Queue;

impl Queue {
    pub fn shuffle(&self) -> bool {
        self.shuffled_play_order.is_some()
    }

    pub fn set_shuffle(&mut self, enabled: bool) {
        if enabled == self.shuffle() {
            return;
        }
        self.shuffled_play_order = enabled.then(|| self.shuffled_order(self.current_index()));
        self.bump_revision();
    }

    pub(super) fn shuffled_order(&mut self, first_index: Option<usize>) -> Vec<usize> {
        let item_count = self.items.len();
        match first_index.filter(|&index| index < item_count) {
            Some(first_index) => self.shuffled_order_starting_with(first_index),
            None => self.shuffled_indexes((0..item_count).collect()),
        }
    }

    fn shuffled_order_starting_with(&mut self, first_index: usize) -> Vec<usize> {
        let following_indexes = (0..self.items.len())
            .filter(|&index| index != first_index)
            .collect();
        let shuffled_following = self.shuffled_indexes(following_indexes);
        iter::once(first_index).chain(shuffled_following).collect()
    }

    pub(super) fn shuffled_indexes(&mut self, mut indexes: Vec<usize>) -> Vec<usize> {
        self.random_generator.shuffle(&mut indexes);
        indexes
    }
}
