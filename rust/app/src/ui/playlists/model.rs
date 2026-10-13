use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};

use feishin_core::model::Playlist;
use slint::{Image, Model, ModelNotify, ModelTracker};

use crate::ui::{PlaylistRow, format_duration};

const COVER_CAPACITY: usize = 48;

fn playlist_row(playlist: &Playlist) -> PlaylistRow {
    PlaylistRow {
        id: playlist.id.as_str().into(),
        name: playlist.name.as_str().into(),
        songs: playlist.song_count.to_string().into(),
        duration: format_duration(playlist.duration_ms).into(),
        owner: playlist.owner.as_deref().unwrap_or_default().into(),
        cover: Image::default(),
        has_cover: false,
    }
}

#[derive(Default)]
struct CoverLru {
    images: HashMap<String, Image>,
    order: VecDeque<String>,
    requested: HashSet<String>,
}

impl CoverLru {
    fn insert(&mut self, cover_id: String, image: Image) {
        self.requested.remove(&cover_id);
        if self.images.insert(cover_id.clone(), image).is_none() {
            self.order.push_back(cover_id);
        }
        while self.order.len() > COVER_CAPACITY {
            if let Some(evicted) = self.order.pop_front() {
                self.images.remove(&evicted);
            }
        }
    }
}

#[derive(Clone)]
struct PlaylistEntry {
    row: PlaylistRow,
    cover_id: Option<String>,
}

pub struct PlaylistsModel {
    entries: RefCell<Vec<PlaylistEntry>>,
    covers: RefCell<CoverLru>,
    generation: Cell<u64>,
    notify: ModelNotify,
    fetch_cover: Box<dyn Fn(String)>,
}

impl PlaylistsModel {
    pub fn new(fetch_cover: impl Fn(String) + 'static) -> Self {
        Self {
            entries: RefCell::default(),
            covers: RefCell::default(),
            generation: Cell::new(0),
            notify: ModelNotify::default(),
            fetch_cover: Box::new(fetch_cover),
        }
    }

    pub fn begin(&self) -> u64 {
        self.generation.set(self.generation.get() + 1);
        self.generation.get()
    }

    pub fn set_playlists(&self, generation: u64, playlists: &[Playlist]) -> bool {
        if generation != self.generation.get() {
            return false;
        }
        *self.entries.borrow_mut() = playlists
            .iter()
            .map(|playlist| PlaylistEntry {
                row: playlist_row(playlist),
                cover_id: playlist.cover_art_id.clone(),
            })
            .collect();
        self.notify.reset();
        true
    }

    pub fn apply_cover(&self, cover_id: String, image: Option<Image>) {
        let Some(image) = image else {
            return;
        };
        let changed: Vec<usize> = self
            .entries
            .borrow()
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.cover_id.as_deref() == Some(cover_id.as_str()))
            .map(|(index, _)| index)
            .collect();
        self.covers.borrow_mut().insert(cover_id, image);
        for row in changed {
            self.notify.row_changed(row);
        }
    }

    fn with_cover(&self, mut row: PlaylistRow, cover_id: String) -> PlaylistRow {
        let cached = self.covers.borrow().images.get(&cover_id).cloned();
        match cached {
            Some(image) => {
                row.cover = image;
                row.has_cover = true;
            }
            None => self.request_cover(cover_id),
        }
        row
    }

    fn request_cover(&self, cover_id: String) {
        let first_request = self.covers.borrow_mut().requested.insert(cover_id.clone());
        if first_request {
            (self.fetch_cover)(cover_id);
        }
    }
}

impl Model for PlaylistsModel {
    type Data = PlaylistRow;

    fn row_count(&self) -> usize {
        self.entries.borrow().len()
    }

    fn row_data(&self, row: usize) -> Option<PlaylistRow> {
        let entry = self.entries.borrow().get(row).cloned()?;
        let Some(cover_id) = entry.cover_id else {
            return Some(entry.row);
        };
        Some(self.with_cover(entry.row, cover_id))
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;

    fn playlist(id: &str, cover: Option<&str>) -> Playlist {
        Playlist {
            id: id.into(),
            server_id: "demo".into(),
            name: id.into(),
            comment: None,
            owner: Some("me".into()),
            public: false,
            song_count: 3,
            duration_ms: 185_000,
            cover_art_id: cover.map(Into::into),
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn rows_map_playlists() {
        let row = playlist_row(&playlist("a", None));
        assert_eq!(row.songs, "3");
        assert_eq!(row.duration, "3:05");
        assert_eq!(row.owner, "me");
    }

    #[test]
    fn covers_are_requested_once_and_stale_loads_ignored() {
        let requested = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&requested);
        let model = PlaylistsModel::new(move |cover_id| sink.borrow_mut().push(cover_id));
        let stale = model.begin();
        let current = model.begin();
        assert!(!model.set_playlists(stale, &[playlist("old", None)]));
        assert!(model.set_playlists(current, &[playlist("a", Some("c1")), playlist("b", None)]));
        assert_eq!(model.row_count(), 2);

        model.row_data(0);
        model.row_data(0);
        model.row_data(1);
        assert_eq!(*requested.borrow(), ["c1"]);

        model.apply_cover("c1".into(), Some(Image::default()));
        assert!(model.row_data(0).is_some_and(|row| row.has_cover));
        assert!(model.row_data(1).is_some_and(|row| !row.has_cover));
    }
}
