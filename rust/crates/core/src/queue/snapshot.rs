use std::collections::HashSet;
use std::mem;

use serde::{Deserialize, Serialize};

use super::{FIRST_UID, Queue, QueueItem, RepeatMode, Uid};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueueSnapshot {
    pub items: Vec<QueueItem>,
    pub shuffled: Option<Vec<usize>>,
    pub current_uid: Option<Uid>,
    pub repeat: RepeatMode,
    pub next_uid: Uid,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QueueView<'queue> {
    pub items: &'queue [QueueItem],
    pub shuffled: Option<&'queue [usize]>,
    pub current_uid: Option<Uid>,
    pub repeat: RepeatMode,
    pub next_uid: Uid,
}

impl QueueSnapshot {
    pub fn view(&self) -> QueueView<'_> {
        QueueView {
            items: &self.items,
            shuffled: self.shuffled.as_deref(),
            current_uid: self.current_uid,
            repeat: self.repeat,
            next_uid: self.next_uid,
        }
    }
}

impl Queue {
    pub fn view(&self) -> QueueView<'_> {
        QueueView {
            items: &self.items,
            shuffled: self.shuffled_play_order.as_deref(),
            current_uid: self.current_uid,
            repeat: self.repeat,
            next_uid: self.next_uid,
        }
    }

    pub fn snapshot(&self) -> QueueSnapshot {
        QueueSnapshot {
            items: self.items.clone(),
            shuffled: self.shuffled_play_order.clone(),
            current_uid: self.current_uid,
            repeat: self.repeat,
            next_uid: self.next_uid,
        }
    }

    pub fn from_snapshot(snapshot: QueueSnapshot) -> Self {
        let QueueSnapshot {
            items,
            shuffled,
            current_uid,
            repeat,
            next_uid,
        } = snapshot;

        let mut queue = Self::new();
        queue.next_uid = first_free_uid(next_uid, &items);
        queue.items = queue.restored_items(items);
        queue.reindex_all();
        queue.repeat = repeat;
        queue.current_uid = current_uid.filter(|&uid| queue.index_of(uid).is_some());
        queue.shuffled_play_order =
            shuffled.map(|play_order| queue.valid_or_regenerated_order(play_order));
        queue
    }

    fn restored_items(&mut self, items: Vec<QueueItem>) -> Vec<QueueItem> {
        let mut seen_uids = HashSet::with_capacity(items.len());
        items
            .into_iter()
            .map(|item| {
                let uid = match seen_uids.insert(item.uid) {
                    true => item.uid,
                    false => self.allocate_uid(),
                };
                let track = self.interner.reintern(item.track);
                QueueItem { uid, track }
            })
            .collect()
    }

    fn valid_or_regenerated_order(&mut self, play_order: Vec<usize>) -> Vec<usize> {
        if is_permutation(&play_order, self.items.len()) {
            return play_order;
        }
        self.shuffled_order(self.current_index())
    }
}

fn first_free_uid(stored_next_uid: Uid, items: &[QueueItem]) -> Uid {
    let after_highest_uid = items
        .iter()
        .map(|item| item.uid)
        .max()
        .map_or(0, |highest_uid| highest_uid.saturating_add(1));
    stored_next_uid.max(after_highest_uid).max(FIRST_UID)
}

pub(super) fn is_permutation(play_order: &[usize], item_count: usize) -> bool {
    if play_order.len() != item_count {
        return false;
    }
    let mut seen_indexes = vec![false; item_count];
    play_order.iter().all(|&item_index| {
        seen_indexes
            .get_mut(item_index)
            .is_some_and(|already_seen| !mem::replace(already_seen, true))
    })
}
