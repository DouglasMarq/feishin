use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use feishin_core::model::Playlist;

#[derive(Default)]
pub struct Picker {
    pub generation: u64,
    song_ids: Vec<String>,
    pub playlists: Vec<Playlist>,
    busy: bool,
}

pub type SharedPicker = Arc<Mutex<Picker>>;

pub fn lock(shared: &SharedPicker) -> MutexGuard<'_, Picker> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

pub struct PendingRequest {
    pub generation: u64,
    pub song_ids: Vec<String>,
}

impl Picker {
    pub fn restart(&mut self, song_ids: Vec<String>) -> u64 {
        *self = Self {
            generation: self.generation + 1,
            song_ids,
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
        if self.busy || self.song_ids.is_empty() {
            return None;
        }
        self.busy = true;
        Some(PendingRequest {
            generation: self.generation,
            song_ids: self.song_ids.clone(),
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
