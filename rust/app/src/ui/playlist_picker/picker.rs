use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use feishin_core::model::Playlist;

use crate::ui::tracks::ListedSongs;

#[derive(Debug, Clone, PartialEq)]
pub enum PickedSongs {
    Ids(Vec<String>),
    Listed(ListedSongs),
}

impl Default for PickedSongs {
    fn default() -> Self {
        Self::Ids(Vec::new())
    }
}

impl PickedSongs {
    pub fn song_count(&self) -> usize {
        match self {
            Self::Ids(song_ids) => song_ids.len(),
            Self::Listed(listed) => listed.song_count(),
        }
    }
}

#[derive(Default)]
pub struct Picker {
    pub generation: u64,
    songs: PickedSongs,
    pub playlists: Vec<Playlist>,
    busy: bool,
}

pub type SharedPicker = Arc<Mutex<Picker>>;

pub fn lock(shared: &SharedPicker) -> MutexGuard<'_, Picker> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

pub struct PendingRequest {
    pub generation: u64,
    pub songs: PickedSongs,
}

impl Picker {
    pub fn restart(&mut self, songs: PickedSongs) -> u64 {
        *self = Self {
            generation: self.generation + 1,
            songs,
            ..Self::default()
        };
        self.generation
    }

    pub fn accept_playlists(
        &mut self,
        generation: u64,
        result: feishin_api::Result<Vec<Playlist>>,
    ) -> Option<feishin_api::Result<()>> {
        if self.generation != generation {
            return None;
        }
        Some(result.map(|playlists| {
            self.playlists = playlists;
        }))
    }

    pub fn playlist_name(&self, playlist_id: &str) -> String {
        self.playlists
            .iter()
            .find(|playlist| playlist.id == playlist_id)
            .map_or_else(|| "playlist".to_owned(), |playlist| playlist.name.clone())
    }

    pub fn start_request(&mut self) -> Option<PendingRequest> {
        if self.busy || self.songs.song_count() == 0 {
            return None;
        }
        self.busy = true;
        Some(PendingRequest {
            generation: self.generation,
            songs: self.songs.clone(),
        })
    }

    pub fn finish_request(&mut self, generation: u64) -> bool {
        if self.generation != generation {
            return false;
        }
        self.busy = false;
        true
    }
}
