mod anchored;
mod indexing;
mod insertion;
mod interleaving;
mod interner;
mod item;
mod lookup;
mod navigation;
mod random;
mod removal;
mod reordering;
mod shuffling;
mod snapshot;
mod track;

#[cfg(test)]
mod tests;

use std::collections::HashMap;

pub use anchored::InsertAnchor;
pub use item::{Advance, QueueItem, RepeatMode, Uid};
pub use snapshot::QueueSnapshot;
pub use track::QueueTrack;

use interner::TrackInterner;
use random::SplitMix64;

const FIRST_UID: Uid = 1;
const FIRST_POSITION: usize = 0;

#[derive(Debug, Clone)]
pub struct Queue {
    items: Vec<QueueItem>,
    uid_indexes: HashMap<Uid, usize>,
    shuffled_play_order: Option<Vec<usize>>,
    current_uid: Option<Uid>,
    repeat: RepeatMode,
    next_uid: Uid,
    revision: u64,
    interner: TrackInterner,
    random_generator: SplitMix64,
}

impl Default for Queue {
    fn default() -> Self {
        Self::new()
    }
}

impl Queue {
    pub fn new() -> Self {
        Self::with_generator(SplitMix64::from_random_seed())
    }

    pub fn with_seed(seed: u64) -> Self {
        Self::with_generator(SplitMix64::from_seed(seed))
    }

    fn with_generator(random_generator: SplitMix64) -> Self {
        Self {
            items: Vec::new(),
            uid_indexes: HashMap::new(),
            shuffled_play_order: None,
            current_uid: None,
            repeat: RepeatMode::None,
            next_uid: FIRST_UID,
            revision: 0,
            interner: TrackInterner::default(),
            random_generator,
        }
    }

    pub fn items(&self) -> &[QueueItem] {
        &self.items
    }

    pub fn display_order(&self) -> Vec<&QueueItem> {
        match &self.shuffled_play_order {
            Some(play_order) => play_order
                .iter()
                .filter_map(|&item_index| self.items.get(item_index))
                .collect(),
            None => self.items.iter().collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn current(&self) -> Option<&QueueItem> {
        self.current_index()
            .and_then(|item_index| self.items.get(item_index))
    }

    pub fn current_position(&self) -> Option<usize> {
        self.current_index()
            .and_then(|item_index| self.position_of_index(item_index))
    }

    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    pub fn set_repeat(&mut self, mode: RepeatMode) {
        self.repeat = mode;
    }
}
