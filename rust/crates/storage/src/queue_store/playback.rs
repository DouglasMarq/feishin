use std::path::Path;

use feishin_core::queue::{QueueSnapshotRef, RepeatMode, Uid};
use serde::{Deserialize, Serialize};

use super::codec::FileFormat;
use super::{PLAYBACK_FILE, StoredQueue, read_if_present};
use crate::Result;

const PLAYBACK_FORMAT: FileFormat = FileFormat {
    file_name: PLAYBACK_FILE,
    magic: *b"FSP1",
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackState {
    pub current_uid: Option<Uid>,
    pub position_ms: u64,
    pub repeat: RepeatMode,
}

impl PlaybackState {
    pub fn of(view: &QueueSnapshotRef<'_>, position_ms: u64) -> Self {
        Self {
            current_uid: view.current_uid,
            position_ms,
            repeat: view.repeat,
        }
    }

    pub(super) fn encode(&self) -> Result<Vec<u8>> {
        PLAYBACK_FORMAT.encode(self)
    }

    fn apply_to(self, mut stored: StoredQueue) -> StoredQueue {
        stored.snapshot.repeat = self.repeat;
        let Some(current_uid) = self.current_uid else {
            return stored;
        };
        if stored
            .snapshot
            .items
            .iter()
            .any(|item| item.uid == current_uid)
        {
            stored.snapshot.current_uid = Some(current_uid);
            stored.position_ms = self.position_ms;
        }
        stored
    }
}

pub(super) fn apply_saved(data_dir: &Path, stored: StoredQueue) -> StoredQueue {
    match load(data_dir) {
        Ok(Some(playback)) => playback.apply_to(stored),
        Ok(None) => stored,
        Err(error) => {
            tracing::warn!("ignoring the saved playback position: {error}");
            stored
        }
    }
}

fn load(data_dir: &Path) -> Result<Option<PlaybackState>> {
    read_if_present(&data_dir.join(PLAYBACK_FILE))?
        .map(|bytes| PLAYBACK_FORMAT.decode(&bytes))
        .transpose()
}
