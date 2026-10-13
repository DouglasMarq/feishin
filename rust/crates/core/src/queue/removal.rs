use std::collections::{HashMap, HashSet};

use super::{Queue, Uid};

const SPARE_CAPACITY_FACTOR: usize = 2;

impl Queue {
    pub fn remove(&mut self, uids: &[Uid]) {
        let removed_uids: HashSet<Uid> = uids.iter().copied().collect();
        let Some(first_removed_index) = self
            .items
            .iter()
            .position(|item| removed_uids.contains(&item.uid))
        else {
            return;
        };
        if self
            .current_uid
            .is_some_and(|uid| removed_uids.contains(&uid))
        {
            self.current_uid = self.first_kept_uid_after_current(&removed_uids);
        }
        self.reindex_shuffled_order_without(&removed_uids);
        self.items.retain(|item| !removed_uids.contains(&item.uid));
        self.uid_indexes
            .retain(|uid, _item_index| !removed_uids.contains(uid));
        self.reindex_from(first_removed_index);
        self.release_spare_capacity();
        self.bump_revision();
        self.interner.release_unused();
    }

    pub fn clear(&mut self) {
        self.items = Vec::new();
        self.uid_indexes = HashMap::new();
        self.interner.clear();
        self.current_uid = None;
        if let Some(play_order) = &mut self.shuffled_play_order {
            play_order.clear();
        }
        self.bump_revision();
    }

    fn release_spare_capacity(&mut self) {
        if self.items.capacity() / SPARE_CAPACITY_FACTOR > self.items.len() {
            self.items.shrink_to_fit();
            self.uid_indexes.shrink_to_fit();
        }
        if let Some(play_order) = &mut self.shuffled_play_order
            && play_order.capacity() / SPARE_CAPACITY_FACTOR > play_order.len()
        {
            play_order.shrink_to_fit();
        }
    }

    fn first_kept_uid_after_current(&self, removed_uids: &HashSet<Uid>) -> Option<Uid> {
        let play_indexes = self.play_indexes();
        let following_position = self.current_position().map_or(0, |position| position + 1);
        let kept_uid = |item_index: &usize| {
            self.uid_at_index(*item_index)
                .filter(|uid| !removed_uids.contains(uid))
        };
        play_indexes
            .iter()
            .skip(following_position)
            .find_map(kept_uid)
            .or_else(|| play_indexes.iter().find_map(kept_uid))
    }

    fn reindex_shuffled_order_without(&mut self, removed_uids: &HashSet<Uid>) {
        let Some(play_order) = self.shuffled_play_order.take() else {
            return;
        };
        let remaining_indexes = self.indexes_after_removal(removed_uids);
        let reindexed_order = play_order
            .into_iter()
            .filter_map(|old_index| remaining_indexes.get(old_index).copied().flatten())
            .collect();
        self.shuffled_play_order = Some(reindexed_order);
    }

    fn indexes_after_removal(&self, removed_uids: &HashSet<Uid>) -> Vec<Option<usize>> {
        self.items
            .iter()
            .scan(0, |next_index, item| {
                let new_index = (!removed_uids.contains(&item.uid)).then(|| {
                    let kept_index = *next_index;
                    *next_index += 1;
                    kept_index
                });
                Some(new_index)
            })
            .collect()
    }
}
