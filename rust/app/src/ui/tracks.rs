use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;

use feishin_api::MusicServer;
use feishin_core::model::{Genre, ItemKind, Paging, Song, SongListQuery, SongSort, SortOrder};
use feishin_storage::settings::{ColumnSetting, UiSettings};
use slint::{
    ComponentHandle, Model, ModelNotify, ModelRc, ModelTracker, SharedString, Timer, TimerMode,
    VecModel,
};

use super::{
    Actions, AppState, AppWindow, Context, Route, TrackRow, TracksState, clamp_i32,
    format_duration, on_ui, show_status,
};
use crate::controller::{Command, artist_names};

pub const PAGE_SIZE: usize = 100;
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(300);
const ALL_GENRES: &str = "All genres";

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

/// Row multi-selection with the usual click, Ctrl+click and Shift+click rules.
#[derive(Debug, Default, Clone)]
pub struct Selection {
    rows: BTreeSet<usize>,
    anchor: Option<usize>,
}

impl Selection {
    pub fn contains(&self, index: usize) -> bool {
        self.rows.contains(&index)
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn rows(&self) -> impl Iterator<Item = usize> + '_ {
        self.rows.iter().copied()
    }

    /// Applies a left click and returns the rows whose selected state changed.
    pub fn click(&mut self, index: usize, toggle: bool, extend: bool) -> Vec<usize> {
        let before = self.rows.clone();
        match self.anchor.filter(|_| extend) {
            Some(anchor) => {
                if !toggle {
                    self.rows.clear();
                }
                self.rows.extend(anchor.min(index)..=anchor.max(index));
            }
            None if toggle => {
                if !self.rows.remove(&index) {
                    self.rows.insert(index);
                }
                self.anchor = Some(index);
            }
            None => {
                self.rows = BTreeSet::from([index]);
                self.anchor = Some(index);
            }
        }
        before.symmetric_difference(&self.rows).copied().collect()
    }

    /// Right click: keeps a selection that already holds `index`, otherwise selects only it.
    pub fn focus(&mut self, index: usize) -> Vec<usize> {
        if self.contains(index) {
            Vec::new()
        } else {
            self.click(index, false, false)
        }
    }
}

/// The selected songs when an action starts; rows on pages not loaded yet are fetched by `resolve`.
pub struct SelectionSnapshot {
    query: SongListQuery,
    rows: Vec<(usize, Option<Song>)>,
}

impl SelectionSnapshot {
    pub fn missing_pages(&self) -> BTreeSet<usize> {
        self.rows
            .iter()
            .filter(|(_, song)| song.is_none())
            .map(|(index, _)| index / PAGE_SIZE)
            .collect()
    }

    pub fn is_loaded(&self) -> bool {
        self.rows.iter().all(|(_, song)| song.is_some())
    }

    /// The songs already in hand, in selection order.
    pub fn into_loaded(self) -> Vec<Song> {
        self.rows.into_iter().filter_map(|(_, song)| song).collect()
    }

    pub async fn resolve(self, server: &dyn MusicServer) -> feishin_api::Result<Vec<Song>> {
        let mut pages = HashMap::new();
        for page in self.missing_pages() {
            let paging = Paging {
                start_index: page * PAGE_SIZE,
                limit: PAGE_SIZE,
            };
            pages.insert(page, server.song_list(&self.query, paging).await?.items);
        }
        Ok(self
            .rows
            .into_iter()
            .filter_map(|(index, song)| {
                song.or_else(|| {
                    pages
                        .get(&(index / PAGE_SIZE))?
                        .get(index % PAGE_SIZE)
                        .cloned()
                })
            })
            .collect())
    }
}

#[derive(Default)]
struct ListState {
    generation: u64,
    query: SongListQuery,
    total: usize,
    pages: HashMap<usize, Vec<Song>>,
    requested: HashSet<usize>,
    current_song: Option<String>,
    selection: Selection,
}

impl ListState {
    fn song(&self, index: usize) -> Option<&Song> {
        self.pages.get(&(index / PAGE_SIZE))?.get(index % PAGE_SIZE)
    }

    fn index_of(&self, song_id: Option<&str>) -> Option<usize> {
        let song_id = song_id?;
        self.pages.iter().find_map(|(page, songs)| {
            songs
                .iter()
                .position(|song| song.id == song_id)
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

    pub fn apply_page(&self, generation: u64, page: usize, songs: Vec<Song>) {
        let range = {
            let mut state = self.state.borrow_mut();
            if state.generation != generation {
                return;
            }
            let start = page * PAGE_SIZE;
            let range = start..(start + songs.len()).min(state.total);
            state.pages.insert(page, songs);
            range
        };
        self.notify_rows(range);
    }

    pub fn page_failed(&self, generation: u64, page: usize) {
        let mut state = self.state.borrow_mut();
        if state.generation == generation {
            state.requested.remove(&page);
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

    /// Updates the selection for a click on `index` and returns the selected count.
    pub fn select(&self, index: usize, toggle: bool, extend: bool) -> usize {
        self.change_selection(index, |selection| selection.click(index, toggle, extend))
    }

    /// Makes sure `index` is selected before a context menu opens; returns the selected count.
    pub fn focus(&self, index: usize) -> usize {
        self.change_selection(index, |selection| selection.focus(index))
    }

    fn change_selection(
        &self,
        index: usize,
        f: impl FnOnce(&mut Selection) -> Vec<usize>,
    ) -> usize {
        let (changed, count) = {
            let mut state = self.state.borrow_mut();
            if index >= state.total {
                return state.selection.len();
            }
            let changed = f(&mut state.selection);
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

    pub fn first_selected(&self) -> Option<Song> {
        let state = self.state.borrow();
        state
            .selection
            .rows()
            .next()
            .and_then(|index| state.song(index).cloned())
    }

    /// Applies `update` to every loaded song whose id is in `ids`.
    pub fn update_songs(&self, ids: &[String], update: impl Fn(&mut Song)) {
        let ids: HashSet<&str> = ids.iter().map(String::as_str).collect();
        let changed = {
            let mut state = self.state.borrow_mut();
            let mut changed = Vec::new();
            for (page, songs) in &mut state.pages {
                for (i, song) in songs.iter_mut().enumerate() {
                    if ids.contains(song.id.as_str()) {
                        update(song);
                        changed.push(page * PAGE_SIZE + i);
                    }
                }
            }
            changed
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
        let page = row / PAGE_SIZE;
        let (selected, request) = {
            let mut state = self.state.borrow_mut();
            if row >= state.total {
                return None;
            }
            let selected = state.selection.contains(row);
            if let Some(song) = state.song(row) {
                return Some(TrackRow {
                    playing: state.current_song.as_deref() == Some(song.id.as_str()),
                    selected,
                    ..track_row(song, row)
                });
            }
            let request = state.requested.insert(page).then(|| Request::Page {
                generation: state.generation,
                query: state.query.clone(),
                page,
            });
            (selected, request)
        };
        if let Some(request) = request {
            (self.fetch)(request);
        }
        Some(TrackRow {
            index: clamp_i32(row as u64 + 1),
            selected,
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
    let artist_id = song
        .artists
        .first()
        .or_else(|| song.album_artists.first())
        .map(|artist| artist.id.as_str())
        .unwrap_or_default();
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
        album_id: song.album_id.as_deref().unwrap_or_default().into(),
        artist_id: artist_id.into(),
        favorite: song.favorite,
        rating: i32::from(song.user_rating.unwrap_or_default()),
        selected: false,
    }
}

/// The filter bar values, applied on top of the sort and search of a `SongListQuery`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filters {
    pub favorite_only: bool,
    pub genre_id: Option<String>,
    pub year_from: Option<i32>,
    pub year_to: Option<i32>,
}

impl Filters {
    pub fn apply(&self, query: SongListQuery) -> SongListQuery {
        let (year_from, year_to) = match (self.year_from, self.year_to) {
            (Some(from), Some(to)) if from > to => (Some(to), Some(from)),
            years => years,
        };
        SongListQuery {
            favorite: self.favorite_only.then_some(true),
            genre_ids: self.genre_id.iter().cloned().collect(),
            year_from,
            year_to,
            ..query
        }
    }
}

pub fn parse_year(text: &str) -> Option<i32> {
    text.trim()
        .parse()
        .ok()
        .filter(|year| (1..=9999).contains(year))
}

fn filters(window: &AppWindow) -> Filters {
    let state = window.global::<TracksState>();
    let genre_id = usize::try_from(state.get_genre_index() - 1)
        .ok()
        .and_then(|i| state.get_genre_ids().row_data(i))
        .map(|id| id.to_string());
    Filters {
        favorite_only: state.get_filter_favorite(),
        genre_id,
        year_from: parse_year(&state.get_year_from()),
        year_to: parse_year(&state.get_year_to()),
    }
}

fn current_query(window: &AppWindow) -> Option<SongListQuery> {
    let query = with_model(window, TracksModel::query)?;
    Some(filters(window).apply(query))
}

fn string_model(items: Vec<SharedString>) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(items))
}

fn set_genres(window: &AppWindow, mut genres: Vec<Genre>) {
    genres.sort_by_cached_key(|genre| genre.name.to_lowercase());
    let names = std::iter::once(ALL_GENRES.into())
        .chain(genres.iter().map(|genre| genre.name.as_str().into()))
        .collect();
    let ids = genres
        .iter()
        .map(|genre| genre.id.as_str().into())
        .collect();
    let state = window.global::<TracksState>();
    state.set_genres(string_model(names));
    state.set_genre_ids(string_model(ids));
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
                            show_status(window, &format!("Could not load tracks: {e}"));
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
                let result = server.song_list(&query, paging).await;
                on_ui(&window, move |window| match result {
                    Ok(songs) => {
                        with_model(window, |model| {
                            model.apply_page(generation, page, songs.items)
                        });
                    }
                    Err(e) => {
                        with_model(window, |model| model.page_failed(generation, page));
                        show_status(window, &format!("Could not load tracks: {e}"));
                    }
                });
            }
        }
    });
}

pub fn sort_for(column: &str) -> SongSort {
    match column {
        "artist" => SongSort::Artist,
        "album" => SongSort::Album,
        "year" => SongSort::Year,
        "plays" => SongSort::PlayCount,
        "duration" => SongSort::Duration,
        "favorite" => SongSort::Favorited,
        "rating" => SongSort::Rating,
        _ => SongSort::Name,
    }
}

fn reload(window: &AppWindow, query: SongListQuery) {
    let state = window.global::<TracksState>();
    state.set_loading(true);
    state.set_selected_count(0);
    with_model(window, |model| model.reset(query));
}

/// Loads the list from scratch for the current sort, search and filters, e.g. after connecting.
pub fn refresh(window: &AppWindow) {
    window.global::<TracksState>().invoke_load_genres();
    if let Some(query) = current_query(window) {
        reload(window, query);
    }
}

pub fn set_current(window: &AppWindow, song_id: Option<&str>) {
    with_model(window, |model| model.set_current(song_id));
}

pub fn columns(window: &AppWindow) -> Vec<ColumnSetting> {
    let state = window.global::<TracksState>();
    [
        ("index", state.get_w_index(), true),
        ("title", state.get_w_title(), true),
        ("artist", state.get_w_artist(), state.get_show_artist()),
        ("album", state.get_w_album(), state.get_show_album()),
        ("year", state.get_w_year(), state.get_show_year()),
        ("plays", state.get_w_plays(), state.get_show_plays()),
        (
            "duration",
            state.get_w_duration(),
            state.get_show_duration(),
        ),
        (
            "favorite",
            state.get_w_favorite(),
            state.get_show_favorite(),
        ),
        ("rating", state.get_w_rating(), state.get_show_rating()),
    ]
    .into_iter()
    .map(|(id, width, visible)| ColumnSetting {
        id: id.to_owned(),
        width,
        visible,
    })
    .collect()
}

fn apply_columns(window: &AppWindow, columns: &[ColumnSetting]) {
    let state = window.global::<TracksState>();
    for column in columns.iter().filter(|c| c.width > 0.0) {
        let (width, visible) = (column.width, column.visible);
        match column.id.as_str() {
            "index" => state.set_w_index(width),
            "title" => state.set_w_title(width),
            "artist" => {
                state.set_w_artist(width);
                state.set_show_artist(visible);
            }
            "album" => {
                state.set_w_album(width);
                state.set_show_album(visible);
            }
            "year" => {
                state.set_w_year(width);
                state.set_show_year(visible);
            }
            "plays" => {
                state.set_w_plays(width);
                state.set_show_plays(visible);
            }
            "duration" => {
                state.set_w_duration(width);
                state.set_show_duration(visible);
            }
            "favorite" => {
                state.set_w_favorite(width);
                state.set_show_favorite(visible);
            }
            "rating" => {
                state.set_w_rating(width);
                state.set_show_rating(visible);
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SongAction {
    Play,
    PlayNext,
    AddLast,
    AddToPlaylist,
    Favorite(bool),
    Rate(u8),
}

fn act_on_selection(window: &AppWindow, ctx: &Context, action: SongAction) {
    let Some(snapshot) = with_model(window, TracksModel::selection) else {
        return;
    };
    if snapshot.is_loaded() {
        perform(window, ctx, action, snapshot.into_loaded());
        return;
    }
    let Some(server) = ctx.session.get() else {
        return;
    };
    let weak = window.as_weak();
    let ctx = ctx.clone();
    ctx.rt.clone().spawn(async move {
        let result = snapshot.resolve(server.as_ref()).await;
        on_ui(&weak, move |window| match result {
            Ok(songs) => perform(window, &ctx, action, songs),
            Err(e) => show_status(window, &format!("Could not load the selection: {e}")),
        });
    });
}

fn perform(window: &AppWindow, ctx: &Context, action: SongAction, songs: Vec<Song>) {
    if songs.is_empty() {
        return;
    }
    match action {
        SongAction::Play => ctx.send(Command::PlaySongs {
            songs,
            song_id: None,
        }),
        SongAction::PlayNext => ctx.send(Command::AddNext(songs)),
        SongAction::AddLast => ctx.send(Command::AddLast(songs)),
        SongAction::AddToPlaylist => {
            let ids = songs.iter().map(|song| song.id.as_str().into()).collect();
            window
                .global::<Actions>()
                .invoke_add_to_playlist(string_model(ids));
        }
        SongAction::Favorite(favorite) => set_favorite(window, ctx, ids_of(&songs), favorite),
        SongAction::Rate(rating) => set_rating(window, ctx, ids_of(&songs), rating),
    }
}

fn ids_of(songs: &[Song]) -> Vec<String> {
    songs.iter().map(|song| song.id.clone()).collect()
}

fn set_favorite(window: &AppWindow, ctx: &Context, ids: Vec<String>, favorite: bool) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let weak = window.as_weak();
    ctx.rt.spawn(async move {
        let result = server.set_favorite(ItemKind::Song, &ids, favorite).await;
        on_ui(&weak, move |window| match result {
            Ok(()) => {
                with_model(window, |model| {
                    model.update_songs(&ids, |song| song.favorite = favorite)
                });
            }
            Err(e) => show_status(window, &format!("Could not update favorites: {e}")),
        });
    });
}

fn set_rating(window: &AppWindow, ctx: &Context, ids: Vec<String>, rating: u8) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let weak = window.as_weak();
    ctx.rt.spawn(async move {
        let mut rated = Vec::new();
        let mut error = None;
        for id in ids {
            match server.set_rating(ItemKind::Song, &id, rating).await {
                Ok(()) => rated.push(id),
                Err(e) => {
                    error = Some(e);
                    break;
                }
            }
        }
        on_ui(&weak, move |window| {
            let value = (rating > 0).then_some(rating);
            with_model(window, |model| {
                model.update_songs(&rated, |song| song.user_rating = value)
            });
            if let Some(e) = error {
                show_status(window, &format!("Could not set the rating: {e}"));
            }
        });
    });
}

fn go_to(window: &AppWindow, route: Route, target: impl FnOnce(&Song) -> Option<String>) {
    let id = with_model(window, TracksModel::first_selected)
        .flatten()
        .and_then(|song| target(&song));
    if let Some(id) = id {
        window
            .global::<AppState>()
            .invoke_navigate(route, id.into());
    }
}

fn install_genres(window: &AppWindow, ctx: &Context) {
    let requested_for: RefCell<Option<String>> = RefCell::default();
    let weak = window.as_weak();
    let ctx = ctx.clone();
    window.global::<TracksState>().on_load_genres(move || {
        let (Some(window), Some(server)) = (weak.upgrade(), ctx.session.get()) else {
            return;
        };
        let server_id = server.config().id.clone();
        if requested_for.borrow().as_deref() == Some(server_id.as_str()) {
            return;
        }
        requested_for.replace(Some(server_id.clone()));
        let state = window.global::<TracksState>();
        state.set_genre_index(0);
        set_genres(&window, Vec::new());
        let weak = weak.clone();
        let session = ctx.session.clone();
        ctx.rt.spawn(async move {
            let result = server.genre_list().await;
            on_ui(&weak, move |window| {
                if session.get().is_none_or(|s| s.config().id != server_id) {
                    return;
                }
                match result {
                    Ok(genres) => set_genres(window, genres),
                    Err(e) => tracing::warn!("loading genres failed: {e}"),
                }
            });
        });
    });
}

fn install_selection(window: &AppWindow, ctx: &Context) {
    let state = window.global::<TracksState>();

    let weak = window.as_weak();
    state.on_select_row(move |index, toggle, extend| {
        let (Some(window), Ok(index)) = (weak.upgrade(), usize::try_from(index)) else {
            return;
        };
        if let Some(count) = with_model(&window, |model| model.select(index, toggle, extend)) {
            window
                .global::<TracksState>()
                .set_selected_count(clamp_i32(count as u64));
        }
    });

    let weak = window.as_weak();
    state.on_context_row(move |index| {
        let (Some(window), Ok(index)) = (weak.upgrade(), usize::try_from(index)) else {
            return;
        };
        if let Some(count) = with_model(&window, |model| model.focus(index)) {
            window
                .global::<TracksState>()
                .set_selected_count(clamp_i32(count as u64));
        }
    });

    let on_action = |action: SongAction| {
        let weak = window.as_weak();
        let ctx = ctx.clone();
        move || {
            if let Some(window) = weak.upgrade() {
                act_on_selection(&window, &ctx, action);
            }
        }
    };
    state.on_play_selected(on_action(SongAction::Play));
    state.on_add_selected_to_playlist(on_action(SongAction::AddToPlaylist));
    let (next, last) = (
        on_action(SongAction::PlayNext),
        on_action(SongAction::AddLast),
    );
    state.on_queue_selected(move |play_next| if play_next { next() } else { last() });
    let (add, remove) = (
        on_action(SongAction::Favorite(true)),
        on_action(SongAction::Favorite(false)),
    );
    state.on_favorite_selected(move |favorite| if favorite { add() } else { remove() });

    let weak = window.as_weak();
    let rate_ctx = ctx.clone();
    state.on_rate_selected(move |rating| {
        if let (Some(window), Ok(rating)) = (weak.upgrade(), u8::try_from(rating.clamp(0, 5))) {
            act_on_selection(&window, &rate_ctx, SongAction::Rate(rating));
        }
    });

    let weak = window.as_weak();
    state.on_go_to_album(move || {
        if let Some(window) = weak.upgrade() {
            go_to(&window, Route::AlbumDetail, |song| song.album_id.clone());
        }
    });

    let weak = window.as_weak();
    state.on_go_to_artist(move || {
        if let Some(window) = weak.upgrade() {
            go_to(&window, Route::ArtistDetail, |song| {
                song.artists
                    .first()
                    .or_else(|| song.album_artists.first())
                    .map(|artist| artist.id.clone())
            });
        }
    });

    let weak = window.as_weak();
    let row_ctx = ctx.clone();
    state.on_toggle_favorite(move |index| {
        let (Some(window), Ok(index)) = (weak.upgrade(), usize::try_from(index)) else {
            return;
        };
        if let Some(song) = with_model(&window, |model| model.song_at(index)).flatten() {
            set_favorite(&window, &row_ctx, vec![song.id], !song.favorite);
        }
    });

    let weak = window.as_weak();
    let row_ctx = ctx.clone();
    state.on_rate_row(move |index, rating| {
        let (Some(window), Ok(index), Ok(rating)) = (
            weak.upgrade(),
            usize::try_from(index),
            u8::try_from(rating.clamp(0, 5)),
        ) else {
            return;
        };
        if let Some(id) = with_model(&window, |model| model.song_id_at(index)).flatten() {
            set_rating(&window, &row_ctx, vec![id], rating);
        }
    });
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
    let debounce = Rc::new(Timer::default());
    state.on_filters_changed(move |debounced| {
        let weak = weak.clone();
        let apply = move || {
            let Some(window) = weak.upgrade() else {
                return;
            };
            if let Some(query) = current_query(&window)
                && Some(&query) != with_model(&window, |model| model.query()).as_ref()
            {
                reload(&window, query);
            }
        };
        if debounced {
            debounce.start(TimerMode::SingleShot, SEARCH_DEBOUNCE, apply);
        } else {
            debounce.stop();
            apply();
        }
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

    install_genres(window, ctx);
    install_selection(window, ctx);
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::demo::{FakeServer, test_song};

    type Log = Rc<RefCell<Vec<(u64, Option<usize>)>>>;

    fn drain(log: &Log) -> Vec<(u64, Option<usize>)> {
        log.borrow_mut().drain(..).collect()
    }

    fn logging_model() -> (TracksModel, Log) {
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
        (model, log)
    }

    fn page_of(page: usize, len: usize) -> Vec<Song> {
        (0..len)
            .map(|i| test_song(&format!("s{}", page * PAGE_SIZE + i), 1000))
            .collect()
    }

    #[test]
    fn pages_are_requested_once_and_stale_generations_ignored() {
        let (model, log) = logging_model();
        model.reset(SongListQuery::default());
        assert_eq!(drain(&log), [(1, None)]);
        assert!(model.apply_count(1, 250));
        assert_eq!(model.row_count(), 250);

        assert!(model.row_data(150).is_some_and(|row| !row.loaded));
        model.row_data(160);
        assert_eq!(drain(&log), [(1, Some(1))]);

        model.apply_page(1, 1, vec![test_song("x", 1000)]);
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

    #[test]
    fn clicks_follow_desktop_selection_rules() {
        let mut selection = Selection::default();
        assert_eq!(selection.click(3, false, false), [3]);
        assert_eq!(selection.click(5, false, true), [4, 5]);
        assert_eq!(selection.rows().collect::<Vec<_>>(), [3, 4, 5]);

        assert_eq!(selection.click(1, false, true), [1, 2, 4, 5]);
        assert_eq!(selection.rows().collect::<Vec<_>>(), [1, 2, 3]);

        assert_eq!(selection.click(8, true, false), [8]);
        assert_eq!(selection.click(2, true, false), [2]);
        assert_eq!(selection.rows().collect::<Vec<_>>(), [1, 3, 8]);

        assert_eq!(selection.click(4, true, true), [2, 4]);
        assert_eq!(selection.rows().collect::<Vec<_>>(), [1, 2, 3, 4, 8]);

        assert!(selection.focus(3).is_empty());
        assert_eq!(selection.focus(9), [1, 2, 3, 4, 8, 9]);
        assert_eq!(selection.len(), 1);

        let mut fresh = Selection::default();
        assert_eq!(fresh.click(6, false, true), [6]);
    }

    #[test]
    fn selection_marks_rows_and_resets_with_the_query() {
        let (model, _log) = logging_model();
        model.reset(SongListQuery::default());
        model.apply_count(1, 250);
        model.apply_page(1, 0, page_of(0, PAGE_SIZE));

        assert_eq!(model.select(2, false, false), 1);
        assert_eq!(model.select(150, false, true), 149);
        assert!(model.row_data(2).is_some_and(|row| row.selected));
        assert!(
            model
                .row_data(120)
                .is_some_and(|row| row.selected && !row.loaded)
        );
        assert!(model.row_data(1).is_some_and(|row| !row.selected));
        assert_eq!(model.select(400, false, false), 149);

        let snapshot = model.selection();
        assert_eq!(
            snapshot.missing_pages().into_iter().collect::<Vec<_>>(),
            [1]
        );
        assert!(!snapshot.is_loaded());
        assert_eq!(model.first_selected().map(|s| s.id), Some("s2".into()));

        model.reset(SongListQuery::default());
        assert_eq!(model.selection().rows.len(), 0);
    }

    #[test]
    fn updates_reach_loaded_rows() {
        let (model, _log) = logging_model();
        model.reset(SongListQuery::default());
        model.apply_count(1, 10);
        model.apply_page(1, 0, page_of(0, 10));
        model.update_songs(&["s4".into()], |song| {
            song.favorite = true;
            song.user_rating = Some(4);
        });
        let row = model.row_data(4).unwrap_or_default();
        assert!(row.favorite);
        assert_eq!(row.rating, 4);
        assert!(model.row_data(3).is_some_and(|row| !row.favorite));
    }

    #[tokio::test]
    async fn selections_over_unloaded_pages_are_fetched() {
        let server = FakeServer::new(500);
        let query = SongListQuery::default();
        let first = server
            .song_list(
                &query,
                Paging {
                    start_index: 0,
                    limit: PAGE_SIZE,
                },
            )
            .await
            .map(|page| page.items)
            .unwrap_or_default();
        let snapshot = SelectionSnapshot {
            query: query.clone(),
            rows: vec![(5, first.get(5).cloned()), (230, None), (231, None)],
        };
        let songs = snapshot.resolve(&server).await.unwrap_or_default();
        let expected = server
            .song_list(
                &query,
                Paging {
                    start_index: 230,
                    limit: 2,
                },
            )
            .await
            .map(|page| page.items)
            .unwrap_or_default();
        assert_eq!(songs.len(), 3);
        assert_eq!(songs[0].id, first[5].id);
        assert_eq!(songs[1..], expected[..]);
    }

    #[test]
    fn filters_feed_the_query() {
        let base = SongListQuery {
            sort: SongSort::Year,
            search: Some("river".into()),
            ..SongListQuery::default()
        };
        let query = Filters {
            favorite_only: true,
            genre_id: Some("genre-2".into()),
            year_from: Some(2000),
            year_to: Some(1990),
        }
        .apply(base.clone());
        assert_eq!(query.favorite, Some(true));
        assert_eq!(query.genre_ids, ["genre-2"]);
        assert_eq!((query.year_from, query.year_to), (Some(1990), Some(2000)));
        assert_eq!(
            (query.sort, query.search.as_deref()),
            (SongSort::Year, Some("river"))
        );

        let cleared = Filters::default().apply(query);
        assert_eq!(cleared, base);

        assert_eq!(parse_year(" 1999 "), Some(1999));
        assert_eq!(parse_year(""), None);
        assert_eq!(parse_year("0"), None);
        assert_eq!(parse_year("abc"), None);
    }

    #[test]
    fn rows_carry_links_favorite_and_rating() {
        let server = FakeServer::new(24);
        let song = Song {
            favorite: true,
            user_rating: Some(3),
            ..server.song_at(13)
        };
        let row = track_row(&song, 13);
        assert_eq!(row.index, 14);
        assert_eq!(row.album_id, "album-1");
        assert!(row.artist_id.starts_with("artist-"));
        assert!(row.favorite);
        assert_eq!(row.rating, 3);
        assert_eq!(sort_for("favorite"), SongSort::Favorited);
        assert_eq!(sort_for("rating"), SongSort::Rating);
    }

    mod headless {
        use std::sync::{Arc, Mutex};
        use std::time::Instant;

        use slint::platform::software_renderer::{
            MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
        };
        use slint::platform::{EventLoopProxy, Platform, WindowAdapter};
        use slint::platform::{PointerEventButton, WindowEvent};
        use slint::{EventLoopError, LogicalPosition, PhysicalSize, PlatformError};
        use tokio::sync::mpsc;

        use super::super::*;
        use crate::connect::DemoConnector;
        use crate::covers::DemoCovers;
        use crate::demo::FakeServer;
        use crate::session::Session;
        use crate::store::JsonStore;
        use crate::ui::{Page, nav};

        type Jobs = Arc<Mutex<Vec<Box<dyn FnOnce() + Send>>>>;

        struct TestPlatform {
            window: Rc<MinimalSoftwareWindow>,
            jobs: Jobs,
        }

        struct Proxy(Jobs);

        impl EventLoopProxy for Proxy {
            fn quit_event_loop(&self) -> Result<(), EventLoopError> {
                Ok(())
            }

            fn invoke_from_event_loop(
                &self,
                event: Box<dyn FnOnce() + Send>,
            ) -> Result<(), EventLoopError> {
                self.0
                    .lock()
                    .map_err(|_| EventLoopError::EventLoopTerminated)?
                    .push(event);
                Ok(())
            }
        }

        impl Platform for TestPlatform {
            fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
                Ok(self.window.clone())
            }

            fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
                Some(Box::new(Proxy(Arc::clone(&self.jobs))))
            }
        }

        const SIZE: PhysicalSize = PhysicalSize::new(1280, 820);

        /// Renders and runs queued UI jobs until `done` holds or a few seconds pass.
        fn pump(window: &MinimalSoftwareWindow, jobs: &Jobs, done: impl Fn() -> bool) -> bool {
            let mut buffer =
                vec![PremultipliedRgbaColor::default(); (SIZE.width * SIZE.height) as usize];
            let deadline = Instant::now() + Duration::from_secs(10);
            while Instant::now() < deadline {
                slint::platform::update_timers_and_animations();
                let queued: Vec<_> = jobs
                    .lock()
                    .map(|mut j| j.drain(..).collect())
                    .unwrap_or_default();
                for job in queued {
                    job();
                }
                window.draw_if_needed(|renderer| {
                    renderer.render(&mut buffer, SIZE.width as usize);
                });
                if done() {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            false
        }

        fn row(app: &AppWindow, index: usize) -> TrackRow {
            app.global::<TracksState>()
                .get_rows()
                .row_data(index)
                .unwrap_or_default()
        }

        #[test]
        fn tracks_view_lists_selects_filters_and_acts_on_demo_songs() {
            let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
            let jobs = Jobs::default();
            if slint::platform::set_platform(Box::new(TestPlatform {
                window: Rc::clone(&window),
                jobs: Arc::clone(&jobs),
            }))
            .is_err()
            {
                return;
            }
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("runtime");
            let (commands, mut received) = mpsc::unbounded_channel();
            let ctx = Context {
                rt: rt.handle().clone(),
                commands,
                session: Session::default(),
                store: Arc::new(JsonStore::new(
                    std::env::temp_dir().join("feishin-rs-ui-test"),
                )),
                connector: Arc::new(DemoConnector),
                covers: Arc::new(DemoCovers),
            };
            ctx.session.set(Some(Arc::new(FakeServer::new(600))));

            let app = AppWindow::new().expect("window");
            install(&app, &ctx, &UiSettings::default());
            nav::install(&app);
            app.show().expect("show");
            window.set_size(SIZE);
            app.global::<AppState>().set_page(Page::Main);
            refresh(&app);

            let state = app.global::<TracksState>();
            assert!(pump(&window, &jobs, || state.get_total() == 600
                && row(&app, 0).loaded));
            assert!(pump(&window, &jobs, || state.get_genres().row_count() == 9));

            let click = |button, y| {
                let position = LogicalPosition::new(400.0, y);
                app.window()
                    .dispatch_event(WindowEvent::PointerPressed { position, button });
                app.window()
                    .dispatch_event(WindowEvent::PointerReleased { position, button });
            };
            click(PointerEventButton::Left, 160.0);
            assert!(row(&app, 0).selected);
            click(PointerEventButton::Right, 304.0);
            assert_eq!(state.get_selected_count(), 1);
            assert!(row(&app, 4).selected && !row(&app, 0).selected);
            assert!(pump(&window, &jobs, || true));

            state.invoke_select_row(0, false, false);
            state.invoke_select_row(2, false, true);
            assert_eq!(state.get_selected_count(), 3);
            assert!(row(&app, 1).selected);
            state.invoke_queue_selected(false);
            assert!(matches!(received.try_recv(), Ok(Command::AddLast(songs)) if songs.len() == 3));

            let album_id = row(&app, 1).album_id;
            state.invoke_context_row(1);
            assert_eq!(state.get_selected_count(), 3);
            state.invoke_select_row(1, false, false);
            state.invoke_go_to_album();
            let nav = app.global::<AppState>();
            assert_eq!(
                (nav.get_route(), nav.get_detail_id()),
                (Route::AlbumDetail, album_id)
            );
            nav.invoke_back();

            let favorite = row(&app, 1).favorite;
            state.invoke_toggle_favorite(1);
            assert!(pump(&window, &jobs, || row(&app, 1).favorite != favorite));

            state.set_filter_favorite(true);
            state.set_genre_index(1);
            state.invoke_filters_changed(false);
            assert!(pump(&window, &jobs, || !state.get_loading()
                && row(&app, 0).loaded));
            assert_eq!(state.get_selected_count(), 0);
            assert!(state.get_total() > 0 && state.get_total() < 600);

            state.invoke_play_row(0);
            match received.try_recv() {
                Ok(Command::PlayFromList { query, song_id }) => {
                    assert_eq!(query.favorite, Some(true));
                    assert_eq!(query.genre_ids.len(), 1);
                    assert_eq!(song_id, row(&app, 0).id.as_str());
                }
                _ => panic!("expected PlayFromList"),
            }
        }
    }
}
