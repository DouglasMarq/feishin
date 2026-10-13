use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

#[derive(Default)]
pub struct Detail {
    pub generation: u64,
    pub id: String,
    pub name: String,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removals_do_not_overlap() {
        let mut detail = Detail::default();
        let generation = detail.restart("playlist-1");
        assert!(detail.start_removal(&[]).is_none());
        assert!(detail.start_removal(&[1]).is_some());
        assert!(detail.start_removal(&[2]).is_none());
        assert!(!detail.finish_removal(generation + 1));
        assert!(detail.finish_removal(generation));
        assert!(detail.start_removal(&[2]).is_some());
    }
}
