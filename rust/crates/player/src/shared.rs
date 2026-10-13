use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, MutexGuard, PoisonError};

use feishin_core::queue::Uid;
use libmpv2::Mpv;

pub(crate) type PlaylistEntryId = i64;

#[derive(Default)]
pub(crate) struct PlaylistEntries {
    pub(crate) uids: HashMap<PlaylistEntryId, Uid>,
    pub(crate) next_entry: Option<PlaylistEntryId>,
}

impl PlaylistEntries {
    pub(crate) fn forget_all(&mut self) {
        self.uids.clear();
        self.next_entry = None;
    }
}

pub(crate) struct Shared {
    pub(crate) mpv: Mpv,
    pub(crate) stop_requested: AtomicBool,
    pub(crate) entries: Mutex<PlaylistEntries>,
}

impl Shared {
    pub(crate) fn lock_entries(&self) -> MutexGuard<'_, PlaylistEntries> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
