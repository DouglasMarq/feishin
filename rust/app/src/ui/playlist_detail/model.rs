use std::cell::RefCell;

use feishin_core::model::Song;
use slint::{Model, ModelNotify, ModelTracker};

use super::pages::{PAGE_SIZE, PageStore, page_of};
use super::rows::{placeholder_row, song_row};
use super::selection::{ClickMode, Selection};
use super::targets::TargetSnapshot;
use crate::ui::PlaylistSongRow;

pub struct PageRequest {
    pub generation: u64,
    pub playlist_id: String,
    pub page: usize,
}

#[derive(Default)]
struct ModelState {
    generation: u64,
    playlist_id: String,
    total: usize,
    pages: PageStore,
    selection: Selection,
}

impl ModelState {
    fn page_request(&mut self, row: usize) -> Option<PageRequest> {
        let page = page_of(row);
        self.pages.request_once(page).then(|| PageRequest {
            generation: self.generation,
            playlist_id: self.playlist_id.clone(),
            page,
        })
    }
}

pub struct PlaylistSongsModel {
    state: RefCell<ModelState>,
    notify: ModelNotify,
    fetch: Box<dyn Fn(PageRequest)>,
}

impl PlaylistSongsModel {
    pub fn new(fetch: impl Fn(PageRequest) + 'static) -> Self {
        Self {
            state: RefCell::default(),
            notify: ModelNotify::default(),
            fetch: Box::new(fetch),
        }
    }

    pub fn reset(&self, generation: u64, playlist_id: &str) {
        *self.state.borrow_mut() = ModelState {
            generation,
            playlist_id: playlist_id.to_owned(),
            ..ModelState::default()
        };
        self.notify.reset();
    }

    pub fn apply_first_page(&self, generation: u64, total: usize, songs: Vec<Song>) -> bool {
        {
            let mut state = self.state.borrow_mut();
            if state.generation != generation {
                return false;
            }
            state.total = total.max(songs.len());
            state.pages.insert(0, songs);
        }
        self.notify.reset();
        true
    }

    pub fn apply_page(&self, generation: u64, page: usize, songs: Vec<Song>) {
        let rows = {
            let mut state = self.state.borrow_mut();
            if state.generation != generation {
                return;
            }
            let start = page * PAGE_SIZE;
            let end = (start + songs.len()).min(state.total);
            match state.pages.insert(page, songs) {
                true => start..end,
                false => 0..0,
            }
        };
        self.notify_rows(rows);
    }

    pub fn page_failed(&self, generation: u64, page: usize) -> bool {
        let mut state = self.state.borrow_mut();
        if state.generation != generation {
            return false;
        }
        state.pages.request_failed(page);
        true
    }

    pub fn total(&self) -> usize {
        self.state.borrow().total
    }

    pub fn song_id_at(&self, index: usize) -> Option<String> {
        let state = self.state.borrow();
        state.pages.song(index).map(|song| song.id.clone())
    }

    pub fn snapshot(&self, indexes: &[usize]) -> TargetSnapshot {
        let state = self.state.borrow();
        TargetSnapshot {
            playlist_id: state.playlist_id.clone(),
            rows: indexes
                .iter()
                .filter(|index| **index < state.total)
                .map(|&index| (index, state.pages.song(index).cloned()))
                .collect(),
        }
    }

    fn notify_rows(&self, rows: impl IntoIterator<Item = usize>) {
        for row in rows {
            self.notify.row_changed(row);
        }
    }
}

impl PlaylistSongsModel {
    pub fn click(&self, index: usize, mode: ClickMode) -> usize {
        self.change_selection(|selection, total| {
            if index < total {
                selection.click(index, mode);
            }
        })
    }

    pub fn select_all(&self) -> usize {
        self.change_selection(Selection::select_all)
    }

    pub fn clear_selection(&self) -> usize {
        self.change_selection(|selection, _| selection.clear())
    }

    pub fn selected_indexes(&self) -> Vec<usize> {
        self.state.borrow().selection.indexes()
    }

    pub fn targets(&self, index: usize) -> Vec<usize> {
        self.state.borrow().selection.targets(index)
    }

    fn change_selection(&self, change: impl FnOnce(&mut Selection, usize)) -> usize {
        let (changed, count) = {
            let mut state = self.state.borrow_mut();
            let before = state.selection.clone();
            let total = state.total;
            change(&mut state.selection, total);
            let changed: Vec<usize> = state.selection.changed_since(&before).collect();
            (changed, state.selection.len())
        };
        self.notify_rows(changed);
        count
    }
}

impl Model for PlaylistSongsModel {
    type Data = PlaylistSongRow;

    fn row_count(&self) -> usize {
        self.state.borrow().total
    }

    fn row_data(&self, row: usize) -> Option<PlaylistSongRow> {
        let (placeholder, request) = {
            let mut state = self.state.borrow_mut();
            if row >= state.total {
                return None;
            }
            state.pages.focus_on(row);
            let selected = state.selection.contains(row);
            if let Some(song) = state.pages.song(row) {
                return Some(song_row(song, row, selected));
            }
            (placeholder_row(row, selected), state.page_request(row))
        };
        if let Some(request) = request {
            (self.fetch)(request);
        }
        Some(placeholder)
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
