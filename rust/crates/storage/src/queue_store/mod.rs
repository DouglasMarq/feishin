mod codec;
mod compact;
mod compact_reader;
mod compact_writer;
mod decoded_tables;
mod legacy;
mod playback;
mod shared_table;
mod short_key_hasher;

#[cfg(test)]
mod tests;

use std::path::Path;

use feishin_core::queue::{QueueSnapshot, QueueView};

use crate::{Result, none_when_missing, write_atomic};

pub use playback::PlaybackState;

pub const QUEUE_FILE: &str = "queue.bin";
pub const PLAYBACK_FILE: &str = "queue-playback.bin";
pub const LEGACY_QUEUE_FILE: &str = "queue.json";

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StoredQueue {
    pub snapshot: QueueSnapshot,
    pub position_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedQueue {
    bytes: Vec<u8>,
    playback: PlaybackState,
}

impl EncodedQueue {
    pub fn byte_count(&self) -> usize {
        self.bytes.len()
    }

    pub fn playback(&self) -> PlaybackState {
        self.playback
    }
}

pub fn encode(view: &QueueView<'_>, position_ms: u64) -> Result<EncodedQueue> {
    Ok(EncodedQueue {
        bytes: compact_writer::encode(view, position_ms)?,
        playback: PlaybackState::of(view, position_ms),
    })
}

pub fn save(data_dir: &Path, encoded: &EncodedQueue) -> Result<()> {
    save_playback(data_dir, &encoded.playback)?;
    write_atomic(&data_dir.join(QUEUE_FILE), &encoded.bytes)?;
    legacy::remove(data_dir);
    Ok(())
}

pub fn save_playback(data_dir: &Path, playback: &PlaybackState) -> Result<()> {
    write_atomic(&data_dir.join(PLAYBACK_FILE), &playback.encode()?)
}

pub fn load(data_dir: &Path) -> Result<Option<StoredQueue>> {
    let Some(stored) = load_queue_file(data_dir)? else {
        return Ok(None);
    };
    Ok(Some(playback::apply_saved(data_dir, stored)))
}

pub fn clear(data_dir: &Path) -> Result<()> {
    for file_name in [QUEUE_FILE, PLAYBACK_FILE, LEGACY_QUEUE_FILE] {
        none_when_missing(std::fs::remove_file(data_dir.join(file_name)))?;
    }
    Ok(())
}

fn load_queue_file(data_dir: &Path) -> Result<Option<StoredQueue>> {
    if let Some(bytes) = read_if_present(&data_dir.join(QUEUE_FILE))? {
        return compact_reader::decode(&bytes).map(Some);
    }
    let legacy_queue = legacy::load(data_dir)?;
    if let Some(stored) = &legacy_queue {
        migrate_legacy_queue(data_dir, stored);
    }
    Ok(legacy_queue)
}

fn migrate_legacy_queue(data_dir: &Path, stored: &StoredQueue) {
    let migrated = encode(&stored.snapshot.view(), stored.position_ms)
        .and_then(|encoded| save(data_dir, &encoded));
    if let Err(error) = migrated {
        tracing::warn!("rewriting the saved queue in the compact format failed: {error}");
    }
}

fn read_if_present(path: &Path) -> Result<Option<Vec<u8>>> {
    none_when_missing(std::fs::read(path))
}
