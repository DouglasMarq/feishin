//! Play queue with Spotify-like semantics.
//!
//! Ported from `src/renderer/store/player.store.ts` plus the `feat/spotify-like-playback` behaviour:
//! - The queue keeps the original order. When shuffle is on, a permutation defines the play order,
//!   and that play order is also the display order (`display_order`).
//! - `play_now` replaces the queue. With shuffle on it reshuffles every time and puts the start song first.
//! - Turning shuffle off restores the original order; turning it on reshuffles with the current item first.
//! - Moving items in the shuffled view only reorders the permutation; the original order is untouched.

use serde::{Deserialize, Serialize};

use crate::model::Song;

/// Stable id of an entry in the queue. The same song can appear more than once with different uids.
pub type Uid = u64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueueItem {
    pub uid: Uid,
    pub song: Song,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RepeatMode {
    #[default]
    None,
    All,
    One,
}

/// Why the player is moving to another item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Advance {
    /// The current track finished on its own. `RepeatMode::One` repeats the same item.
    Auto,
    /// The user pressed next. `RepeatMode::One` still moves on.
    Manual,
}

/// Serializable state used to restore the queue on startup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueueSnapshot {
    pub items: Vec<QueueItem>,
    /// Play order as indexes into `items`; present only while shuffle is on.
    pub shuffled: Option<Vec<usize>>,
    pub current_uid: Option<Uid>,
    pub repeat: RepeatMode,
    pub next_uid: Uid,
}

#[derive(Debug, Clone)]
pub struct Queue {
    _private: (),
}

impl Default for Queue {
    fn default() -> Self {
        Self::new()
    }
}

impl Queue {
    pub fn new() -> Self {
        todo!()
    }

    /// Same as `new` but with a deterministic shuffle, for tests.
    pub fn with_seed(_seed: u64) -> Self {
        todo!()
    }

    /// Items in their original (unshuffled) order.
    pub fn items(&self) -> &[QueueItem] {
        todo!()
    }

    /// Items in play order: the shuffled order while shuffle is on, the original order otherwise.
    pub fn display_order(&self) -> Vec<&QueueItem> {
        todo!()
    }

    pub fn len(&self) -> usize {
        todo!()
    }

    pub fn is_empty(&self) -> bool {
        todo!()
    }

    pub fn current(&self) -> Option<&QueueItem> {
        todo!()
    }

    /// Position of the current item in `display_order`.
    pub fn current_position(&self) -> Option<usize> {
        todo!()
    }

    pub fn shuffle(&self) -> bool {
        todo!()
    }

    pub fn set_shuffle(&mut self, _on: bool) {
        todo!()
    }

    pub fn repeat(&self) -> RepeatMode {
        todo!()
    }

    pub fn set_repeat(&mut self, _mode: RepeatMode) {
        todo!()
    }

    /// Replace the queue with `songs` and make `start_song_id` (or the first song) current.
    /// With shuffle on, the queue is reshuffled with the start song first.
    pub fn play_now(
        &mut self,
        _songs: Vec<Song>,
        _start_song_id: Option<&str>,
    ) -> Option<&QueueItem> {
        todo!()
    }

    /// Insert right after the current item in play order.
    pub fn add_next(&mut self, _songs: Vec<Song>) {
        todo!()
    }

    /// Append at the end of play order.
    pub fn add_last(&mut self, _songs: Vec<Song>) {
        todo!()
    }

    /// Move to the next item in play order, honouring repeat. Returns the new current item.
    pub fn next(&mut self, _advance: Advance) -> Option<&QueueItem> {
        todo!()
    }

    /// Move to the previous item in play order (wraps with `RepeatMode::All`).
    pub fn previous(&mut self) -> Option<&QueueItem> {
        todo!()
    }

    /// The item that `next(Advance::Auto)` would return, for gapless preloading. Does not mutate.
    pub fn peek_next(&self) -> Option<&QueueItem> {
        todo!()
    }

    pub fn play_uid(&mut self, _uid: Uid) -> Option<&QueueItem> {
        todo!()
    }

    pub fn remove(&mut self, _uids: &[Uid]) {
        todo!()
    }

    /// Move `uids` (keeping their relative order) before `before_uid` in play order, or to the end when `None`.
    pub fn move_items(&mut self, _uids: &[Uid], _before_uid: Option<Uid>) {
        todo!()
    }

    pub fn clear(&mut self) {
        todo!()
    }

    pub fn snapshot(&self) -> QueueSnapshot {
        todo!()
    }

    pub fn from_snapshot(_snapshot: QueueSnapshot) -> Self {
        todo!()
    }
}
