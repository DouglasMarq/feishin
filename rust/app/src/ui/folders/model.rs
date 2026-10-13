use std::cell::RefCell;

use feishin_core::model::{Folder, FolderId, Song};
use slint::{Model, ModelNotify, ModelTracker};

use crate::ui::page_cache::{PageCache, PageShape};
use crate::ui::tracks::track_row;
use crate::ui::{FolderListRow, TrackRow, clamp_i32};

pub const SONG_PAGE_SIZE: usize = 100;
const SONG_PAGES: PageShape = PageShape {
    page_size: SONG_PAGE_SIZE,
    capacity: 8,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongPageRequest {
    pub generation: u64,
    pub folder: FolderId,
    pub page: usize,
}

struct Listing {
    generation: u64,
    folder: FolderId,
    folders: Vec<Folder>,
    song_total: usize,
    songs: PageCache<Song>,
}

impl Listing {
    fn empty(generation: u64, folder: FolderId) -> Self {
        Self {
            generation,
            folder,
            folders: Vec::new(),
            song_total: 0,
            songs: PageCache::new(SONG_PAGES),
        }
    }

    fn song_position(&self, row: usize) -> Option<usize> {
        row.checked_sub(self.folders.len())
            .filter(|position| *position < self.song_total)
    }

    fn page_requests(&mut self, position: usize) -> Vec<SongPageRequest> {
        self.songs
            .pages_to_fetch(position, self.song_total)
            .into_iter()
            .map(|page| SongPageRequest {
                generation: self.generation,
                folder: self.folder.clone(),
                page,
            })
            .collect()
    }
}

fn folder_row(folder: &Folder) -> FolderListRow {
    FolderListRow {
        is_folder: true,
        folder_id: folder.id.as_str().into(),
        name: folder.name.as_str().into(),
        track: TrackRow::default(),
    }
}

fn song_row(track: TrackRow) -> FolderListRow {
    FolderListRow {
        is_folder: false,
        folder_id: "".into(),
        name: track.title.clone(),
        track,
    }
}

fn placeholder_track(position: usize) -> TrackRow {
    TrackRow {
        index: clamp_i32(position as u64 + 1),
        ..TrackRow::default()
    }
}

pub struct FolderRowsModel {
    state: RefCell<Listing>,
    notify: ModelNotify,
    fetch: Box<dyn Fn(SongPageRequest)>,
}

impl FolderRowsModel {
    pub fn new(fetch: impl Fn(SongPageRequest) + 'static) -> Self {
        Self {
            state: RefCell::new(Listing::empty(0, FolderId::Root)),
            notify: ModelNotify::default(),
            fetch: Box::new(fetch),
        }
    }

    pub fn reset(&self, folder: FolderId) -> u64 {
        let generation = {
            let mut state = self.state.borrow_mut();
            let generation = state.generation + 1;
            *state = Listing::empty(generation, folder);
            generation
        };
        self.notify.reset();
        generation
    }

    pub fn is_current(&self, generation: u64) -> bool {
        self.state.borrow().generation == generation
    }

    pub fn show(
        &self,
        generation: u64,
        folders: Vec<Folder>,
        song_total: usize,
        first_songs: Vec<Song>,
    ) -> bool {
        {
            let mut state = self.state.borrow_mut();
            if state.generation != generation {
                return false;
            }
            state.folders = folders;
            state.song_total = song_total.max(first_songs.len());
            state.songs.insert(0, first_songs);
        }
        self.notify.reset();
        true
    }

    pub fn apply_song_page(&self, request: &SongPageRequest, songs: Vec<Song>) {
        let rows = {
            let mut state = self.state.borrow_mut();
            if state.generation != request.generation {
                return;
            }
            let first_position = state.songs.first_index_of(request.page);
            let first_row = state.folders.len() + first_position;
            let row_count = songs.len();
            state.songs.insert(request.page, songs);
            first_row..first_row + row_count
        };
        for row in rows {
            self.notify.row_changed(row);
        }
    }

    pub fn song_page_failed(&self, request: &SongPageRequest) -> bool {
        let mut state = self.state.borrow_mut();
        if state.generation != request.generation {
            return false;
        }
        state.songs.request_failed(request.page);
        true
    }

    pub fn folder(&self) -> FolderId {
        self.state.borrow().folder.clone()
    }

    pub fn song_at(&self, row: usize) -> Option<(usize, Song)> {
        let state = self.state.borrow();
        let position = state.song_position(row)?;
        state
            .songs
            .get(position)
            .map(|song| (position, song.clone()))
    }
}

impl Model for FolderRowsModel {
    type Data = FolderListRow;

    fn row_count(&self) -> usize {
        let state = self.state.borrow();
        state.folders.len() + state.song_total
    }

    fn row_data(&self, row: usize) -> Option<FolderListRow> {
        let (row_data, requests) = {
            let mut state = self.state.borrow_mut();
            if let Some(folder) = state.folders.get(row) {
                return Some(folder_row(folder));
            }
            let position = state.song_position(row)?;
            let requests = state.page_requests(position);
            let track = state.songs.get(position).map_or_else(
                || placeholder_track(position),
                |song| track_row(song, position),
            );
            (song_row(track), requests)
        };
        for request in requests {
            (self.fetch)(request);
        }
        Some(row_data)
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
