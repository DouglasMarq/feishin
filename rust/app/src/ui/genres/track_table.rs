use std::cell::RefCell;
use std::collections::HashMap;

use feishin_core::model::{Song, SongListQuery};
use slint::{Model, ModelNotify, ModelTracker};

use crate::ui::TrackItem;
use crate::ui::tracks::{PAGE_SIZE, Request, TracksModel};

pub struct TrackTable {
    inner: TracksModel,
    notify: ModelNotify,
    favorite: bool,
    overrides: RefCell<HashMap<String, bool>>,
}

impl TrackTable {
    pub fn new(favorite: bool, fetch: impl Fn(Request) + 'static) -> Self {
        Self {
            inner: TracksModel::new(fetch),
            notify: ModelNotify::default(),
            favorite,
            overrides: RefCell::default(),
        }
    }

    pub fn query(&self) -> SongListQuery {
        self.inner.query()
    }

    pub fn reset(&self, query: SongListQuery) {
        self.overrides.borrow_mut().clear();
        self.inner.reset(query);
        self.notify.reset();
    }

    pub fn apply_count(&self, generation: u64, total: usize) -> bool {
        let applied = self.inner.apply_count(generation, total);
        if applied {
            self.notify.reset();
        }
        applied
    }

    pub fn apply_page(&self, generation: u64, page: usize, songs: Vec<Song>) {
        let start = page * PAGE_SIZE;
        let end = (start + songs.len()).min(self.inner.row_count());
        self.inner.apply_page(generation, page, songs);
        for row in start..end {
            self.notify.row_changed(row);
        }
    }

    pub fn page_failed(&self, generation: u64, page: usize) {
        self.inner.page_failed(generation, page);
    }

    pub fn song_id_at(&self, index: usize) -> Option<String> {
        self.inner.song_id_at(index)
    }

    fn favorite_of(&self, song_id: &str) -> bool {
        self.overrides
            .borrow()
            .get(song_id)
            .copied()
            .unwrap_or(self.favorite)
    }

    pub fn favorite_at(&self, index: usize) -> Option<(String, bool)> {
        let song_id = self.song_id_at(index)?;
        let favorite = self.favorite_of(&song_id);
        Some((song_id, favorite))
    }

    pub fn set_favorite(&self, index: usize, song_id: &str, favorite: bool) {
        self.overrides
            .borrow_mut()
            .insert(song_id.to_owned(), favorite);
        if self.song_id_at(index).as_deref() == Some(song_id) {
            self.notify.row_changed(index);
        }
    }
}

impl Model for TrackTable {
    type Data = TrackItem;

    fn row_count(&self) -> usize {
        self.inner.row_count()
    }

    fn row_data(&self, row: usize) -> Option<TrackItem> {
        let track = self.inner.row_data(row)?;
        let favorite = self.favorite_of(track.id.as_str());
        Some(TrackItem { track, favorite })
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
    use super::*;
    use crate::demo::test_song;

    #[test]
    fn track_table_favorites_flip_and_reset() {
        let table = TrackTable::new(true, |_| {});
        table.reset(SongListQuery::default());
        assert!(table.apply_count(1, 2));
        let songs = [test_song("a", 1000), test_song("b", 1000)];
        table.apply_page(1, 0, songs.to_vec());
        assert_eq!(table.favorite_at(1), Some(("b".to_owned(), true)));
        table.set_favorite(1, "b", false);
        assert!(table.row_data(1).is_some_and(|item| !item.favorite));
        assert!(table.row_data(0).is_some_and(|item| item.favorite));
        table.reset(SongListQuery::default());
        assert!(table.apply_count(2, 2));
        table.apply_page(2, 0, songs.to_vec());
        assert_eq!(table.favorite_at(1), Some(("b".to_owned(), true)));
    }
}
