//! Persists the play queue between runs (`queue.json` in the data dir).

use feishin_core::queue::QueueSnapshot;

use crate::{Paths, Result};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StoredQueue {
    pub snapshot: QueueSnapshot,
    pub position_ms: u64,
}

pub fn save(_paths: &Paths, _queue: &StoredQueue) -> Result<()> {
    todo!()
}

pub fn load(_paths: &Paths) -> Result<Option<StoredQueue>> {
    todo!()
}
