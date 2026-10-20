use std::path::Path;

use super::{LEGACY_QUEUE_FILE, StoredQueue, read_if_present};
use crate::{Result, StorageError, none_when_missing};

pub(super) fn load(data_dir: &Path) -> Result<Option<StoredQueue>> {
    let Some(bytes) = read_if_present(&data_dir.join(LEGACY_QUEUE_FILE))? else {
        return Ok(None);
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| StorageError::malformed(LEGACY_QUEUE_FILE, error))
}

pub(super) fn remove(data_dir: &Path) {
    if let Err(error) = none_when_missing(std::fs::remove_file(data_dir.join(LEGACY_QUEUE_FILE))) {
        tracing::warn!("removing the old {LEGACY_QUEUE_FILE} failed: {error}");
    }
}
