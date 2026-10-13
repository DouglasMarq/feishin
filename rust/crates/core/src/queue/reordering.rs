use std::collections::HashSet;
use std::mem;

use super::{Queue, QueueItem, Uid};

enum Destination {
    Before(usize),
    End,
}

impl Queue {
    pub fn move_items(&mut self, uids: &[Uid], before_uid: Option<Uid>) {
        let moving_uids: HashSet<Uid> = uids.iter().copied().collect();
        let play_indexes = self.play_indexes();
        let Some(destination) = self.destination(before_uid, &play_indexes, &moving_uids) else {
            return;
        };

        let (moved_indexes, mut stationary_indexes): (Vec<usize>, Vec<usize>) = play_indexes
            .iter()
            .partition(|&&item_index| self.is_moving(item_index, &moving_uids));
        if moved_indexes.is_empty() {
            return;
        }
        let insert_position = match destination {
            Destination::Before(anchor_index) => stationary_indexes
                .iter()
                .position(|&item_index| item_index == anchor_index)
                .unwrap_or(stationary_indexes.len()),
            Destination::End => stationary_indexes.len(),
        };
        stationary_indexes.splice(insert_position..insert_position, moved_indexes);
        self.apply_play_order(stationary_indexes);
    }

    fn destination(
        &self,
        before_uid: Option<Uid>,
        play_indexes: &[usize],
        moving_uids: &HashSet<Uid>,
    ) -> Option<Destination> {
        let Some(before_uid) = before_uid else {
            return Some(Destination::End);
        };
        let target_index = self.index_of(before_uid)?;
        let target_position = play_indexes
            .iter()
            .position(|&item_index| item_index == target_index)?;
        let anchor_index = play_indexes
            .iter()
            .skip(target_position)
            .copied()
            .find(|&item_index| !self.is_moving(item_index, moving_uids));
        Some(anchor_index.map_or(Destination::End, Destination::Before))
    }

    fn is_moving(&self, item_index: usize, moving_uids: &HashSet<Uid>) -> bool {
        self.uid_at_index(item_index)
            .is_some_and(|uid| moving_uids.contains(&uid))
    }

    fn apply_play_order(&mut self, play_order: Vec<usize>) {
        if self.shuffle() {
            self.shuffled_play_order = Some(play_order);
            return;
        }
        let mut previous_items: Vec<Option<QueueItem>> =
            mem::take(&mut self.items).into_iter().map(Some).collect();
        self.items = play_order
            .iter()
            .filter_map(|&item_index| previous_items.get_mut(item_index).and_then(Option::take))
            .collect();
    }
}
