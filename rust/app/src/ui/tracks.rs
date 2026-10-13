use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;

use feishin_core::model::{Paging, Song, SongListQuery, SongSort, SortOrder};
use feishin_storage::settings::{ColumnSetting, UiSettings};
use slint::{ComponentHandle, Model, ModelNotify, ModelRc, ModelTracker, Timer, TimerMode};

use super::{AppWindow, Context, TrackRow, TracksState, clamp_i32, format_duration, on_ui};
use crate::controller::{Command, artist_names};

pub const PAGE_SIZE: usize = 100;
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(300);

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

#[derive(Default)]
struct ListState {
    generation: u64,
    query: SongListQuery,
    total: usize,
    pages: HashMap<usize, Vec<TrackRow>>,
    requested: HashSet<usize>,
    current_song: Option<String>,
}

impl ListState {
    fn index_of(&self, song_id: Option<&str>) -> Option<usize> {
        let song_id = song_id?;
        self.pages.iter().find_map(|(page, rows)| {
            rows.iter()
                .position(|row| row.id.as_str() == song_id)
                .map(|i| page * PAGE_SIZE + i)
        })
    }
}

/// Virtualized song list: knows the total from `song_count` and fetches `song_list` pages on demand.
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

    pub fn apply_page(&self, generation: u64, page: usize, rows: Vec<TrackRow>) {
        let range = {
            let mut state = self.state.borrow_mut();
            if state.generation != generation {
                return;
            }
            let start = page * PAGE_SIZE;
            let range = start..(start + rows.len()).min(state.total);
            state.pages.insert(page, rows);
            range
        };
        for row in range {
            self.notify.row_changed(row);
        }
    }

    pub fn page_failed(&self, generation: u64, page: usize) {
        let mut state = self.state.borrow_mut();
        if state.generation == generation {
            state.requested.remove(&page);
        }
    }

    pub fn song_id_at(&self, index: usize) -> Option<String> {
        let state = self.state.borrow();
        let row = state
            .pages
            .get(&(index / PAGE_SIZE))?
            .get(index % PAGE_SIZE)?;
        Some(row.id.to_string())
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
        for row in changed.into_iter().flatten() {
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
        let page = row / PAGE_SIZE;
        let request = {
            let mut state = self.state.borrow_mut();
            if row >= state.total {
                return None;
            }
            if let Some(track) = state
                .pages
                .get(&page)
                .and_then(|rows| rows.get(row % PAGE_SIZE))
            {
                let mut track = track.clone();
                track.playing = state.current_song.as_deref() == Some(track.id.as_str());
                return Some(track);
            }
            state.requested.insert(page).then(|| Request::Page {
                generation: state.generation,
                query: state.query.clone(),
                page,
            })
        };
        if let Some(request) = request {
            (self.fetch)(request);
        }
        Some(TrackRow {
            index: clamp_i32(row as u64 + 1),
            ..TrackRow::default()
        })
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub fn track_row(song: &Song, index: usize) -> TrackRow {
    TrackRow {
        id: song.id.as_str().into(),
        index: clamp_i32(index as u64 + 1),
        title: song.title.as_str().into(),
        artists: artist_names(song).into(),
        album: song.album.as_deref().unwrap_or_default().into(),
        duration: format_duration(song.duration_ms).into(),
        year: song.year.map(|y| y.to_string()).unwrap_or_default().into(),
        plays: song.play_count.to_string().into(),
        loaded: true,
        playing: false,
    }
}

fn with_model<R>(window: &AppWindow, f: impl FnOnce(&TracksModel) -> R) -> Option<R> {
    let rows = window.global::<TracksState>().get_rows();
    rows.as_any().downcast_ref::<TracksModel>().map(f)
}

fn spawn_request(window: slint::Weak<AppWindow>, ctx: &Context, request: Request) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    ctx.rt.spawn(async move {
        match request {
            Request::Count { generation, query } => {
                let result = server.song_count(&query).await;
                on_ui(&window, move |window| {
                    let total = result.as_ref().map_or(0, |total| *total);
                    if with_model(window, |model| model.apply_count(generation, total))
                        == Some(true)
                    {
                        let state = window.global::<TracksState>();
                        state.set_total(clamp_i32(total as u64));
                        state.set_loading(false);
                        if let Err(e) = result {
                            super::show_status(window, &format!("Could not load tracks: {e}"));
                        }
                    }
                });
            }
            Request::Page {
                generation,
                query,
                page,
            } => {
                let paging = Paging {
                    start_index: page * PAGE_SIZE,
                    limit: PAGE_SIZE,
                };
                let result = server.song_list(&query, paging).await.map(|songs| {
                    songs
                        .items
                        .iter()
                        .enumerate()
                        .map(|(i, song)| track_row(song, paging.start_index + i))
                        .collect::<Vec<_>>()
                });
                on_ui(&window, move |window| match result {
                    Ok(rows) => {
                        with_model(window, |model| model.apply_page(generation, page, rows));
                    }
                    Err(e) => {
                        with_model(window, |model| model.page_failed(generation, page));
                        super::show_status(window, &format!("Could not load tracks: {e}"));
                    }
                });
            }
        }
    });
}

fn sort_for(column: &str) -> SongSort {
    match column {
        "artist" => SongSort::Artist,
        "album" => SongSort::Album,
        "year" => SongSort::Year,
        "plays" => SongSort::PlayCount,
        "duration" => SongSort::Duration,
        _ => SongSort::Name,
    }
}

fn reload(window: &AppWindow, query: SongListQuery) {
    window.global::<TracksState>().set_loading(true);
    with_model(window, |model| model.reset(query));
}

/// Loads the list from scratch for the current sort and search, e.g. after connecting.
pub fn refresh(window: &AppWindow) {
    if let Some(query) = with_model(window, |model| model.query()) {
        reload(window, query);
    }
}

pub fn set_current(window: &AppWindow, song_id: Option<&str>) {
    with_model(window, |model| model.set_current(song_id));
}

const COLUMN_IDS: [&str; 7] = [
    "index", "title", "artist", "album", "year", "plays", "duration",
];

pub fn columns(window: &AppWindow) -> Vec<ColumnSetting> {
    let state = window.global::<TracksState>();
    let widths = [
        state.get_w_index(),
        state.get_w_title(),
        state.get_w_artist(),
        state.get_w_album(),
        state.get_w_year(),
        state.get_w_plays(),
        state.get_w_duration(),
    ];
    COLUMN_IDS
        .iter()
        .zip(widths)
        .map(|(id, width)| ColumnSetting {
            id: (*id).to_owned(),
            width,
            visible: true,
        })
        .collect()
}

fn apply_columns(window: &AppWindow, columns: &[ColumnSetting]) {
    let state = window.global::<TracksState>();
    for column in columns.iter().filter(|c| c.width > 0.0) {
        match column.id.as_str() {
            "index" => state.set_w_index(column.width),
            "title" => state.set_w_title(column.width),
            "artist" => state.set_w_artist(column.width),
            "album" => state.set_w_album(column.width),
            "year" => state.set_w_year(column.width),
            "plays" => state.set_w_plays(column.width),
            "duration" => state.set_w_duration(column.width),
            _ => {}
        }
    }
}

pub fn install(window: &AppWindow, ctx: &Context, ui: &UiSettings) {
    apply_columns(window, &ui.track_columns);
    let state = window.global::<TracksState>();
    let weak = window.as_weak();
    let fetch_ctx = ctx.clone();
    let model = Rc::new(TracksModel::new(move |request| {
        spawn_request(weak.clone(), &fetch_ctx, request)
    }));
    state.set_rows(ModelRc::from(model));

    let weak = window.as_weak();
    state.on_sort_by(move |column| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let state = window.global::<TracksState>();
        let desc = state.get_sort_column() == column && !state.get_sort_desc();
        state.set_sort_column(column.clone());
        state.set_sort_desc(desc);
        if let Some(mut query) = with_model(&window, |model| model.query()) {
            query.sort = sort_for(&column);
            query.order = if desc {
                SortOrder::Desc
            } else {
                SortOrder::Asc
            };
            reload(&window, query);
        }
    });

    let weak = window.as_weak();
    let debounce = Rc::new(Timer::default());
    state.on_search_edited(move |text| {
        let weak = weak.clone();
        debounce.start(TimerMode::SingleShot, SEARCH_DEBOUNCE, move || {
            let Some(window) = weak.upgrade() else {
                return;
            };
            if let Some(mut query) = with_model(&window, |model| model.query()) {
                let search = text.trim();
                query.search = (!search.is_empty()).then(|| search.to_owned());
                reload(&window, query);
            }
        });
    });

    let weak = window.as_weak();
    let play_ctx = ctx.clone();
    state.on_play_row(move |index| {
        let (Some(window), Ok(index)) = (weak.upgrade(), usize::try_from(index)) else {
            return;
        };
        let target = with_model(&window, |model| {
            model
                .song_id_at(index)
                .map(|song_id| (model.query(), song_id))
        });
        if let Some(Some((query, song_id))) = target {
            play_ctx.send(Command::PlayFromList { query, song_id });
        }
    });
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    type Log = Rc<RefCell<Vec<(u64, Option<usize>)>>>;

    fn drain(log: &Log) -> Vec<(u64, Option<usize>)> {
        log.borrow_mut().drain(..).collect()
    }

    #[test]
    fn pages_are_requested_once_and_stale_generations_ignored() {
        let log: Log = Rc::default();
        let sink = Rc::clone(&log);
        let model = TracksModel::new(move |request| {
            sink.borrow_mut().push(match request {
                Request::Count { generation, .. } => (generation, None),
                Request::Page {
                    generation, page, ..
                } => (generation, Some(page)),
            })
        });
        model.reset(SongListQuery::default());
        assert_eq!(drain(&log), [(1, None)]);
        assert!(model.apply_count(1, 250));
        assert_eq!(model.row_count(), 250);

        assert!(model.row_data(150).is_some_and(|row| !row.loaded));
        model.row_data(160);
        assert_eq!(drain(&log), [(1, Some(1))]);

        let song = crate::demo::test_song("x", 1000);
        model.apply_page(1, 1, vec![track_row(&song, 100)]);
        model.set_current(Some("x"));
        assert!(
            model
                .row_data(100)
                .is_some_and(|row| row.loaded && row.playing)
        );
        assert_eq!(model.song_id_at(100).as_deref(), Some("x"));

        model.reset(SongListQuery::default());
        assert!(!model.apply_count(1, 10));
        assert_eq!(model.row_count(), 0);
    }
}
