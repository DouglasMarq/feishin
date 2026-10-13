use super::{Queue, QueueItem, Uid};

impl Queue {
    pub(super) fn index_of(&self, uid: Uid) -> Option<usize> {
        self.uid_indexes.get(&uid).copied()
    }

    pub(super) fn current_index(&self) -> Option<usize> {
        self.current_uid.and_then(|uid| self.index_of(uid))
    }

    pub(super) fn position_of_index(&self, item_index: usize) -> Option<usize> {
        match &self.shuffled_play_order {
            Some(_) => self.layout().position_of_index(item_index),
            None => (item_index < self.items.len()).then_some(item_index),
        }
    }

    pub(super) fn item_at_position(&self, position: usize) -> Option<&QueueItem> {
        let item_index = match &self.shuffled_play_order {
            Some(play_order) => *play_order.get(position)?,
            None => position,
        };
        self.items.get(item_index)
    }

    pub(super) fn uid_at_position(&self, position: usize) -> Option<Uid> {
        self.item_at_position(position).map(|item| item.uid)
    }

    pub(super) fn uid_at_index(&self, item_index: usize) -> Option<Uid> {
        self.items.get(item_index).map(|item| item.uid)
    }

    pub(super) fn play_indexes(&self) -> Vec<usize> {
        match &self.shuffled_play_order {
            Some(play_order) => play_order.clone(),
            None => (0..self.items.len()).collect(),
        }
    }
}
