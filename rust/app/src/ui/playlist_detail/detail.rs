use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use feishin_core::model::Song;

use super::selection::Selection;

#[derive(Default)]
pub struct Detail {
    pub generation: u64,
    pub id: String,
    pub name: String,
    pub songs: Vec<Song>,
    pub selection: Selection,
    busy: bool,
}

pub type SharedDetail = Arc<Mutex<Detail>>;

pub fn lock(shared: &SharedDetail) -> MutexGuard<'_, Detail> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

pub struct RemovalTarget {
    pub generation: u64,
    pub playlist_id: String,
    pub name: String,
}

impl Detail {
    pub fn restart(&mut self, playlist_id: &str) -> u64 {
        *self = Self {
            generation: self.generation + 1,
            id: playlist_id.to_owned(),
            ..Self::default()
        };
        self.generation
    }

    pub fn songs_at(&self, indexes: &[usize]) -> Vec<Song> {
        indexes
            .iter()
            .filter_map(|&index| self.songs.get(index).cloned())
            .collect()
    }

    pub fn start_removal(&mut self, indexes: &[usize]) -> Option<RemovalTarget> {
        if self.busy || indexes.is_empty() || self.id.is_empty() {
            return None;
        }
        self.busy = true;
        Some(RemovalTarget {
            generation: self.generation,
            playlist_id: self.id.clone(),
            name: self.name.clone(),
        })
    }

    pub fn finish_removal(&mut self, generation: u64) -> bool {
        if self.generation != generation {
            return false;
        }
        self.busy = false;
        true
    }
}
