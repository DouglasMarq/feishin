use std::cell::RefCell;

use feishin_core::model::Song;

use slint::Image;

use super::super::cover_lru::CoverLru;

const ALBUM_COVER_CAPACITY: usize = 48;

pub struct CurrentArtist {
    pub generation: u64,
    pub id: String,
    pub is_loaded: bool,
    pub is_favorite: bool,
    pub top_songs: Vec<Song>,
    pub album_covers: Vec<Option<String>>,
    pub covers: CoverLru<usize, Image>,
}

impl Default for CurrentArtist {
    fn default() -> Self {
        Self {
            generation: 0,
            id: String::new(),
            is_loaded: false,
            is_favorite: false,
            top_songs: Vec::new(),
            album_covers: Vec::new(),
            covers: CoverLru::new(ALBUM_COVER_CAPACITY),
        }
    }
}

thread_local! {
    static CURRENT_ARTIST: RefCell<CurrentArtist> = RefCell::default();
}

pub fn read_current<Output>(action: impl FnOnce(&CurrentArtist) -> Output) -> Output {
    CURRENT_ARTIST.with_borrow(action)
}

pub fn update_current<Output>(action: impl FnOnce(&mut CurrentArtist) -> Output) -> Output {
    CURRENT_ARTIST.with_borrow_mut(action)
}

pub fn begin(artist_id: &str) -> Option<u64> {
    update_current(|current| {
        if artist_id.is_empty() || current.id == artist_id {
            return None;
        }
        *current = CurrentArtist {
            generation: current.generation + 1,
            id: artist_id.to_owned(),
            ..CurrentArtist::default()
        };
        Some(current.generation)
    })
}

pub fn is_current(generation: u64) -> bool {
    read_current(|current| current.generation == generation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generations_guard_navigation() {
        let first = begin("artist-1");
        assert_eq!(begin("artist-1"), None);
        let second = begin("artist-2");
        assert!(first.is_some() && second > first);
        assert!(first.is_some_and(|generation| !is_current(generation)));
        assert!(second.is_some_and(is_current));
        assert_eq!(begin(""), None);
    }
}
