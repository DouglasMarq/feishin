use std::fmt;
use std::sync::Arc;

use super::{Queue, QueueItem, Uid};

const UNPLACED: usize = usize::MAX;

#[derive(Debug, Default, PartialEq)]
pub(super) struct QueueLayout {
    items: Box<[QueueItem]>,
    play_order: Option<Box<[usize]>>,
    positions_by_index: Option<Box<[usize]>>,
    revision: u64,
}

impl QueueLayout {
    pub(super) fn of(queue: &Queue) -> Self {
        let play_order = queue.shuffled_play_order.as_deref();
        Self {
            items: Box::from(queue.items.as_slice()),
            play_order: play_order.map(Box::from),
            positions_by_index: play_order
                .map(|play_order| positions_by_index(play_order, queue.items.len())),
            revision: queue.revision,
        }
    }

    pub(super) fn position_of_index(&self, item_index: usize) -> Option<usize> {
        match &self.positions_by_index {
            Some(positions) => positions
                .get(item_index)
                .copied()
                .filter(|&position| position != UNPLACED),
            None => (item_index < self.items.len()).then_some(item_index),
        }
    }

    fn item_at_position(&self, position: usize) -> Option<&QueueItem> {
        let item_index = match &self.play_order {
            Some(play_order) => *play_order.get(position)?,
            None => position,
        };
        self.items.get(item_index)
    }

    fn retained_bytes(&self) -> usize {
        let order_length = self.play_order.as_deref().map_or(0, <[usize]>::len);
        let positions_length = self.positions_by_index.as_deref().map_or(0, <[usize]>::len);
        size_of_val(self.items.as_ref()) + (order_length + positions_length) * size_of::<usize>()
    }
}

fn positions_by_index(play_order: &[usize], item_count: usize) -> Box<[usize]> {
    let mut positions = vec![UNPLACED; item_count];
    for (position, &item_index) in play_order.iter().enumerate() {
        if let Some(slot) = positions.get_mut(item_index) {
            *slot = position;
        }
    }
    positions.into_boxed_slice()
}

#[derive(Clone, Default, PartialEq)]
pub struct QueueView {
    layout: Arc<QueueLayout>,
    current_position: Option<usize>,
}

impl QueueView {
    pub(super) fn new(layout: Arc<QueueLayout>, current_position: Option<usize>) -> Self {
        Self {
            layout,
            current_position,
        }
    }

    pub fn len(&self) -> usize {
        self.layout.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.layout.items.is_empty()
    }

    pub fn revision(&self) -> u64 {
        self.layout.revision
    }

    pub fn current_position(&self) -> Option<usize> {
        self.current_position
    }

    pub fn item_at(&self, position: usize) -> Option<&QueueItem> {
        self.layout.item_at_position(position)
    }

    pub fn current(&self) -> Option<&QueueItem> {
        self.item_at(self.current_position?)
    }

    pub fn uids(&self) -> impl Iterator<Item = Uid> + '_ {
        (0..self.len()).filter_map(|position| self.item_at(position).map(|item| item.uid))
    }

    pub fn shares_layout_with(&self, other: &QueueView) -> bool {
        Arc::ptr_eq(&self.layout, &other.layout)
    }

    pub fn retained_bytes(&self) -> usize {
        size_of::<QueueLayout>() + self.layout.retained_bytes()
    }
}

impl fmt::Debug for QueueView {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QueueView")
            .field("revision", &self.layout.revision)
            .field("len", &self.len())
            .field("shuffled", &self.layout.play_order.is_some())
            .field("current_position", &self.current_position)
            .finish()
    }
}
