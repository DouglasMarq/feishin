use feishin_core::queue::Uid;

use super::Player;
use crate::error::{PlayerError, Result};
use crate::ffi::{self, Node};
use crate::shared::{PlaylistEntries, PlaylistEntryId};
use crate::time::mpv_seconds_from_milliseconds;

#[derive(Clone, Copy)]
enum LoadMode {
    Replace,
    Append,
}

impl LoadMode {
    fn mpv_value(self) -> &'static str {
        match self {
            LoadMode::Replace => "replace",
            LoadMode::Append => "append",
        }
    }
}

impl Player {
    pub fn load(&self, uid: Uid, url: &str, start_ms: u64) -> Result<()> {
        let mut entries = self.shared.lock_entries();
        let start_option =
            (start_ms > 0).then(|| format!("start={}", mpv_seconds_from_milliseconds(start_ms)));
        let entry = self.loadfile(url, LoadMode::Replace, start_option.as_deref())?;
        entries.forget_all();
        entries.uids.insert(entry, uid);
        drop(entries);
        self.set("pause", false)
    }

    pub fn preload_next(&self, uid: Uid, url: &str) -> Result<()> {
        let mut entries = self.shared.lock_entries();
        self.clear_next_while_locked(&mut entries)?;
        let entry = self.loadfile(url, LoadMode::Append, None)?;
        entries.uids.insert(entry, uid);
        entries.next_entry = Some(entry);
        Ok(())
    }

    pub fn clear_next(&self) -> Result<()> {
        let mut entries = self.shared.lock_entries();
        self.clear_next_while_locked(&mut entries)
    }

    pub fn stop(&self) -> Result<()> {
        let mut entries = self.shared.lock_entries();
        self.command(&["stop"])?;
        entries.forget_all();
        Ok(())
    }

    fn clear_next_while_locked(&self, entries: &mut PlaylistEntries) -> Result<()> {
        self.command(&["playlist-clear"])?;
        if let Some(previous_next_entry) = entries.next_entry.take()
            && !self.playlist_contains(previous_next_entry)
        {
            entries.uids.remove(&previous_next_entry);
        }
        Ok(())
    }

    fn playlist_contains(&self, entry: PlaylistEntryId) -> bool {
        match ffi::get_property_node(self.shared.mpv.ctx, "playlist") {
            Ok(Node::Array(items)) => items
                .iter()
                .any(|item| item.get("id").and_then(Node::as_i64) == Some(entry)),
            _ => false,
        }
    }

    fn loadfile(
        &self,
        url: &str,
        mode: LoadMode,
        options: Option<&str>,
    ) -> Result<PlaylistEntryId> {
        let mode = mode.mpv_value();
        let reply = match options {
            None => self.command(&["loadfile", url, mode])?,
            Some(options) => self
                .command(&["loadfile", url, mode, "-1", options])
                .or_else(|_| self.command(&["loadfile", url, mode, options]))?,
        };
        reply
            .get("playlist_entry_id")
            .and_then(Node::as_i64)
            .ok_or_else(|| PlayerError::Mpv("loadfile returned no playlist entry id".into()))
    }
}
