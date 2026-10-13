use serde::{Deserialize, Serialize};

use super::QueueTrack;

pub type Uid = u64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueueItem {
    pub uid: Uid,
    #[serde(alias = "song")]
    pub track: QueueTrack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RepeatMode {
    #[default]
    None,
    All,
    One,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Advance {
    Auto,
    Manual,
}
