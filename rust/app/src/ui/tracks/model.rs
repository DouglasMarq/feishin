use std::cell::RefCell;
use std::collections::HashSet;

use feishin_core::model::{Paging, Song, SongListQuery};
use slint::{Model, ModelNotify, ModelTracker};

use super::row::track_row;
use super::selection::{Selection, SelectionSnapshot};
use crate::ui::page_cache::{PageCache, PageShape};
use crate::ui::{TrackRow, clamp_i32};

pub const PAGE_SIZE: usize = 100;
const CACHED_PAGES: usize = 24;
const TRACK_PAGES: PageShape = PageShape {
    page_size: PAGE_SIZE,
    capacity: CACHED_PAGES,
};

pub enum Request {
    Count {
        generation: u64,
        query: SongListQuery,
    },
    Page {
        generation: u64,
        query: SongListQuery,
        page: usize,
    },
}

pub(super) fn paging_for_page(page: usize) -> Paging {
    Paging {
        start_index: page * PAGE_SIZE,
        limit: PAGE_SIZE,
    }
}

struct ListState {
    generation: u64,
    query: SongListQuery,
    total: usize,
    pages: PageCache<Song>,
    current_song: Option<String>,
    selection: Selection,
}

impl Default for ListState {
    fn default() -> Self {
        Self {
            generation: 0,
            query: SongListQuery::default(),
            total: 0,
            pages: PageCache::new(TRACK_PAGES),
            current_song: None,
            selection: Selection::default(),
        }
    }
}

impl ListState {
    fn song(&self, index: usize) -> Option<&Song> {
        self.pages.get(index)
    }

    fn index_of(&self, song_id: Option<&str>) -> Option<usize> {
        let song_id = song_id?;
        self.pages
            .rows()
            .find_map(|(index, song)| (song.id == song_id).then_some(index))
    }

    fn loaded_row(&self, row: usize) -> Option<TrackRow> {
        let song = self.song(row)?;
        Some(TrackRow {
            playing: self.current_song.as_deref() == Some(song.id.as_str()),
            selected: self.selection.contains(row),
            ..track_row(song, row)
        })
    }

    fn visible_row(&self, row: usize) -> TrackRow {
        self.loaded_row(row)
            .unwrap_or_else(|| placeholder_row(row, self.selection.contains(row)))
    }

    fn page_requests_for(&mut self, row: usize) -> Vec<Request> {
        let total = self.total;
        self.pages
            .pages_to_fetch(row, total)
            .into_iter()
            .map(|page| Request::Page {
                generation: self.generation,
                query: self.query.clone(),
                page,
            })
            .collect()
    }

    fn rows_matching(&mut self, ids: &HashSet<&str>) -> Vec<(usize, &mut Song)> {
        self.pages
            .rows_mut()
            .filter(|(_, song)| ids.contains(song.id.as_str()))
            .collect()
    }
}

fn placeholder_row(row: usize, selected: bool) -> TrackRow {
    TrackRow {
        index: clamp_i32(row as u64 + 1),
        selected,
        ..TrackRow::default()
    }
}

pub struct TracksModel {
    state: RefCell<ListState>,
    notify: ModelNotify,
    fetch: Box<dyn Fn(Request)>,
}

impl TracksModel {
    pub fn new(fetch: impl Fn(Request) + 'static) -> Self {
        Self {
            state: RefCell::default(),
            notify: ModelNotify::default(),
            fetch: Box::new(fetch),
        }
    }

    pub fn query(&self) -> SongListQuery {
        self.state.borrow().query.clone()
    }

    pub fn reset(&self, query: SongListQuery) {
        let generation = {
            let mut state = self.state.borrow_mut();
            let current_song = state.current_song.take();
            *state = ListState {
                generation: state.generation + 1,
                query: query.clone(),
                current_song,
                ..ListState::default()
            };
            state.generation
        };
        self.notify.reset();
        (self.fetch)(Request::Count { generation, query });
    }

    pub fn apply_count(&self, generation: u64, total: usize) -> bool {
        {
            let mut state = self.state.borrow_mut();
            if state.generation != generation {
                return false;
            }
            state.total = total;
        }
        self.notify.reset();
        true
    }

    pub fn apply_page(&self, generation: u64, page: usize, songs: Vec<Song>) {
        let range = {
            let mut state = self.state.borrow_mut();
            if state.generation != generation {
                return;
            }
            let start = state.pages.first_index_of(page);
            let range = start..(start + songs.len()).min(state.total);
            state.pages.insert(page, songs);
            range
        };
        self.notify_rows(range);
    }

    pub fn page_failed(&self, generation: u64, page: usize) {
        let mut state = self.state.borrow_mut();
        if state.generation == generation {
            state.pages.forget_request(page);
        }
    }

    pub fn song_at(&self, index: usize) -> Option<Song> {
        self.state.borrow().song(index).cloned()
    }

    pub fn song_id_at(&self, index: usize) -> Option<String> {
        self.state.borrow().song(index).map(|song| song.id.clone())
    }

    pub fn set_current(&self, song_id: Option<&str>) {
        let changed = {
            let mut state = self.state.borrow_mut();
            if state.current_song.as_deref() == song_id {
                return;
            }
            let previous = state.index_of(state.current_song.as_deref());
            state.current_song = song_id.map(str::to_owned);
            [previous, state.index_of(song_id)]
        };
        self.notify_rows(changed.into_iter().flatten());
    }

    pub fn select(&self, index: usize, toggle: bool, extend: bool) -> usize {
        self.change_selection(index, |selection| selection.click(index, toggle, extend))
    }

    pub fn focus(&self, index: usize) -> usize {
        self.change_selection(index, |selection| selection.focus(index))
    }

    fn change_selection(
        &self,
        index: usize,
        change: impl FnOnce(&mut Selection) -> Vec<usize>,
    ) -> usize {
        let (changed, count) = {
            let mut state = self.state.borrow_mut();
            if index >= state.total {
                return state.selection.len();
            }
            let changed = change(&mut state.selection);
            (changed, state.selection.len())
        };
        self.notify_rows(changed);
        count
    }

    pub fn selection(&self) -> SelectionSnapshot {
        let state = self.state.borrow();
        SelectionSnapshot {
            query: state.query.clone(),
            rows: state
                .selection
                .rows()
                .map(|index| (index, state.song(index).cloned()))
                .collect(),
        }
    }

    pub fn update_songs(&self, ids: &[String], update: impl Fn(&mut Song)) {
        let ids: HashSet<&str> = ids.iter().map(String::as_str).collect();
        let changed: Vec<usize> = {
            let mut state = self.state.borrow_mut();
            state
                .rows_matching(&ids)
                .into_iter()
                .map(|(index, song)| {
                    update(song);
                    index
                })
                .collect()
        };
        self.notify_rows(changed);
    }

    fn notify_rows(&self, rows: impl IntoIterator<Item = usize>) {
        for row in rows {
            self.notify.row_changed(row);
        }
    }
}

impl Model for TracksModel {
    type Data = TrackRow;

    fn row_count(&self) -> usize {
        self.state.borrow().total
    }

    fn row_data(&self, row: usize) -> Option<TrackRow> {
        let (visible, requests) = {
            let mut state = self.state.borrow_mut();
            if row >= state.total {
                return None;
            }
            let requests = state.page_requests_for(row);
            (state.visible_row(row), requests)
        };
        for request in requests {
            (self.fetch)(request);
        }
        Some(visible)
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
