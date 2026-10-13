use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use feishin_api::MusicServer;
use feishin_core::model::{
    Album, AlbumListQuery, Artist, ArtistListQuery, Genre, ItemKind, Paging, SongListQuery,
};
use slint::{ComponentHandle, Image, Model, ModelNotify, ModelRc, ModelTracker, VecModel};

use super::player::to_image;
use super::tracks::{PAGE_SIZE, Request, TracksModel, track_row};
use super::{
    AppState, AppWindow, Card, CardRow, Context, GenreRow, GenresState, TrackItem, TrackRow, on_ui,
    show_status,
};
use crate::controller::Command;

pub const GRID_PAGE: usize = 60;
const COVER_SIZE: u32 = 200;
const COVER_CAPACITY: usize = 96;

/// Decoded covers shared by every card grid, bounded so long grids never hold the whole library.
struct CoverLru {
    images: HashMap<String, Image>,
    order: VecDeque<String>,
    capacity: usize,
}

impl CoverLru {
    fn new(capacity: usize) -> Self {
        Self {
            images: HashMap::new(),
            order: VecDeque::new(),
            capacity,
        }
    }

    fn get(&mut self, id: &str) -> Option<Image> {
        let image = self.images.get(id)?.clone();
        if let Some(key) = self
            .order
            .iter()
            .position(|key| key == id)
            .and_then(|pos| self.order.remove(pos))
        {
            self.order.push_back(key);
        }
        Some(image)
    }

    fn insert(&mut self, id: String, image: Image) {
        if self.images.insert(id.clone(), image).is_none() {
            self.order.push_back(id);
        }
        while self.order.len() > self.capacity {
            if let Some(evicted) = self.order.pop_front() {
                self.images.remove(&evicted);
            }
        }
    }
}

thread_local! {
    static COVERS: RefCell<CoverLru> = RefCell::new(CoverLru::new(COVER_CAPACITY));
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardData {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub cover_id: Option<String>,
    pub favorite: bool,
}

pub fn album_card(album: &Album) -> CardData {
    let artists = if album.album_artists.is_empty() {
        &album.artists
    } else {
        &album.album_artists
    };
    CardData {
        id: album.id.clone(),
        title: album.name.clone(),
        subtitle: artists
            .iter()
            .map(|artist| artist.name.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        cover_id: album.cover_art_id.clone(),
        favorite: album.favorite,
    }
}

pub fn artist_card(artist: &Artist) -> CardData {
    CardData {
        id: artist.id.clone(),
        title: artist.name.clone(),
        subtitle: match artist.album_count {
            Some(1) => "1 album".to_owned(),
            Some(count) => format!("{count} albums"),
            None => String::new(),
        },
        cover_id: artist.cover_art_id.clone(),
        favorite: artist.favorite,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardSource {
    Albums(AlbumListQuery),
    Artists(ArtistListQuery),
}

impl CardSource {
    pub fn kind(&self) -> ItemKind {
        match self {
            Self::Albums(_) => ItemKind::Album,
            Self::Artists(_) => ItemKind::Artist,
        }
    }

    async fn fetch(
        &self,
        server: &dyn MusicServer,
        paging: Paging,
    ) -> feishin_api::Result<(Vec<CardData>, Option<usize>)> {
        Ok(match self {
            Self::Albums(query) => {
                let page = server.album_list(query, paging).await?;
                (
                    page.items.iter().map(album_card).collect(),
                    page.total_count,
                )
            }
            Self::Artists(query) => {
                let page = server.artist_list(query, paging).await?;
                (
                    page.items.iter().map(artist_card).collect(),
                    page.total_count,
                )
            }
        })
    }
}

pub enum GridRequest {
    Page {
        generation: u64,
        source: CardSource,
        page: usize,
    },
    Cover {
        cover_id: String,
    },
}

#[derive(Default)]
struct GridState {
    generation: u64,
    source: Option<CardSource>,
    columns: usize,
    total: usize,
    pages: HashMap<usize, Vec<CardData>>,
    requested: HashSet<usize>,
    covers_requested: HashSet<String>,
    overrides: HashMap<String, bool>,
}

impl GridState {
    fn columns(&self) -> usize {
        self.columns.max(1)
    }

    fn rows(&self) -> usize {
        self.total.div_ceil(self.columns())
    }

    fn cards(&self) -> impl Iterator<Item = (usize, &CardData)> {
        self.pages.iter().flat_map(|(page, cards)| {
            cards
                .iter()
                .enumerate()
                .map(move |(i, card)| (page * GRID_PAGE + i, card))
        })
    }
}

/// Virtualized card grid: rows of `columns` cards, pages fetched on demand and covers loaded lazily
/// for the rows the view asks for. A page without a total extends the grid while pages come back full.
pub struct CardGrid {
    state: RefCell<GridState>,
    notify: ModelNotify,
    fetch: Box<dyn Fn(GridRequest)>,
}

impl CardGrid {
    pub fn new(fetch: impl Fn(GridRequest) + 'static) -> Self {
        Self {
            state: RefCell::default(),
            notify: ModelNotify::default(),
            fetch: Box::new(fetch),
        }
    }

    /// Starts over with `source`, or empties the grid when `None`.
    pub fn reset(&self, source: Option<CardSource>) {
        let request = {
            let mut state = self.state.borrow_mut();
            *state = GridState {
                generation: state.generation + 1,
                columns: state.columns,
                source: source.clone(),
                ..GridState::default()
            };
            source.map(|source| {
                state.requested.insert(0);
                GridRequest::Page {
                    generation: state.generation,
                    source,
                    page: 0,
                }
            })
        };
        self.notify.reset();
        if let Some(request) = request {
            (self.fetch)(request);
        }
    }

    pub fn kind(&self) -> Option<ItemKind> {
        self.state.borrow().source.as_ref().map(CardSource::kind)
    }

    pub fn set_columns(&self, columns: usize) {
        {
            let mut state = self.state.borrow_mut();
            if state.columns == columns.max(1) {
                return;
            }
            state.columns = columns.max(1);
        }
        self.notify.reset();
    }

    /// Returns false for a response to an older `reset`.
    pub fn apply_page(
        &self,
        generation: u64,
        page: usize,
        cards: Vec<CardData>,
        total: Option<usize>,
    ) -> bool {
        let (old_rows, new_rows, changed) = {
            let mut state = self.state.borrow_mut();
            if state.generation != generation {
                return false;
            }
            let start = page * GRID_PAGE;
            let end = start + cards.len();
            let old_rows = state.rows();
            state.total = match total {
                Some(total) => total,
                None if cards.len() == GRID_PAGE => state.total.max(end + 1),
                None => end,
            };
            state.pages.insert(page, cards);
            let columns = state.columns();
            (
                old_rows,
                state.rows(),
                start / columns..end.div_ceil(columns),
            )
        };
        if new_rows > old_rows {
            self.notify.row_added(old_rows, new_rows - old_rows);
        } else if new_rows < old_rows {
            self.notify.row_removed(new_rows, old_rows - new_rows);
        }
        for row in changed.start..changed.end.min(old_rows.min(new_rows)) {
            self.notify.row_changed(row);
        }
        true
    }

    /// Returns false for a response to an older `reset`.
    pub fn page_failed(&self, generation: u64, page: usize) -> bool {
        let mut state = self.state.borrow_mut();
        state.generation == generation && state.requested.remove(&page)
    }

    /// A failed cover (`None`) stays marked as requested so it is not retried until the next reset.
    pub fn apply_cover(&self, cover_id: &str, image: Option<Image>) {
        let Some(image) = image else {
            return;
        };
        COVERS.with_borrow_mut(|covers| covers.insert(cover_id.to_owned(), image));
        let rows: HashSet<usize> = {
            let mut state = self.state.borrow_mut();
            state.covers_requested.remove(cover_id);
            let columns = state.columns();
            state
                .cards()
                .filter(|(_, card)| card.cover_id.as_deref() == Some(cover_id))
                .map(|(index, _)| index / columns)
                .collect()
        };
        for row in rows {
            self.notify.row_changed(row);
        }
    }

    pub fn favorite(&self, id: &str) -> Option<bool> {
        let state = self.state.borrow();
        let card = state.cards().find(|(_, card)| card.id == id)?.1;
        Some(state.overrides.get(id).copied().unwrap_or(card.favorite))
    }

    pub fn set_favorite(&self, id: &str, favorite: bool) {
        let row = {
            let mut state = self.state.borrow_mut();
            state.overrides.insert(id.to_owned(), favorite);
            let columns = state.columns();
            state
                .cards()
                .find(|(_, card)| card.id == id)
                .map(|(index, _)| index / columns)
        };
        if let Some(row) = row {
            self.notify.row_changed(row);
        }
    }
}

impl Model for CardGrid {
    type Data = CardRow;

    fn row_count(&self) -> usize {
        self.state.borrow().rows()
    }

    fn row_data(&self, row: usize) -> Option<CardRow> {
        let mut requests = Vec::new();
        let cards = {
            let mut guard = self.state.borrow_mut();
            let state = &mut *guard;
            let columns = state.columns();
            let start = row * columns;
            if start >= state.total {
                return None;
            }
            let end = (start + columns).min(state.total);
            let mut cards = Vec::with_capacity(end - start);
            for index in start..end {
                let page = index / GRID_PAGE;
                let Some(data) = state
                    .pages
                    .get(&page)
                    .and_then(|cards| cards.get(index % GRID_PAGE))
                else {
                    if let Some(source) = &state.source
                        && state.requested.insert(page)
                    {
                        requests.push(GridRequest::Page {
                            generation: state.generation,
                            source: source.clone(),
                            page,
                        });
                    }
                    cards.push(Card::default());
                    continue;
                };
                let cover = data
                    .cover_id
                    .as_deref()
                    .and_then(|id| COVERS.with_borrow_mut(|covers| covers.get(id)));
                if cover.is_none()
                    && let Some(id) = &data.cover_id
                    && state.covers_requested.insert(id.clone())
                {
                    requests.push(GridRequest::Cover {
                        cover_id: id.clone(),
                    });
                }
                cards.push(Card {
                    id: data.id.as_str().into(),
                    title: data.title.as_str().into(),
                    subtitle: data.subtitle.as_str().into(),
                    has_cover: cover.is_some(),
                    cover: cover.unwrap_or_default(),
                    favorite: state
                        .overrides
                        .get(&data.id)
                        .copied()
                        .unwrap_or(data.favorite),
                    loaded: true,
                });
            }
            cards
        };
        for request in requests {
            (self.fetch)(request);
        }
        Some(CardRow {
            cards: ModelRc::new(VecModel::from(cards)),
        })
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// `TracksModel` paging plus a per-row favorite flag that unfavoriting flips in place.
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

    pub fn apply_page(&self, generation: u64, page: usize, rows: Vec<TrackRow>) {
        let start = page * PAGE_SIZE;
        let end = (start + rows.len()).min(self.inner.row_count());
        self.inner.apply_page(generation, page, rows);
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

    pub fn favorite_at(&self, index: usize) -> Option<(String, bool)> {
        let id = self.song_id_at(index)?;
        let favorite = self
            .overrides
            .borrow()
            .get(&id)
            .copied()
            .unwrap_or(self.favorite);
        Some((id, favorite))
    }

    pub fn set_favorite(&self, index: usize, id: &str, favorite: bool) {
        self.overrides.borrow_mut().insert(id.to_owned(), favorite);
        if self.song_id_at(index).as_deref() == Some(id) {
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
        let favorite = self
            .overrides
            .borrow()
            .get(track.id.as_str())
            .copied()
            .unwrap_or(self.favorite);
        Some(TrackItem { track, favorite })
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Where a view keeps a list model and its loading flag.
pub struct Binding<T: 'static> {
    pub rows: fn(&AppWindow) -> ModelRc<T>,
    pub loading: fn(&AppWindow, bool),
}

impl<T> Clone for Binding<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Binding<T> {}

pub fn with_table<R>(
    window: &AppWindow,
    binding: Binding<TrackItem>,
    f: impl FnOnce(&TrackTable) -> R,
) -> Option<R> {
    let rows = (binding.rows)(window);
    rows.as_any().downcast_ref::<TrackTable>().map(f)
}

pub fn with_grid<R>(
    window: &AppWindow,
    binding: Binding<CardRow>,
    f: impl FnOnce(&CardGrid) -> R,
) -> Option<R> {
    let rows = (binding.rows)(window);
    rows.as_any().downcast_ref::<CardGrid>().map(f)
}

pub fn track_table(
    window: &AppWindow,
    ctx: &Context,
    favorite: bool,
    binding: Binding<TrackItem>,
) -> ModelRc<TrackItem> {
    let weak = window.as_weak();
    let ctx = ctx.clone();
    ModelRc::new(TrackTable::new(favorite, move |request| {
        spawn_tracks(weak.clone(), &ctx, binding, request)
    }))
}

pub fn card_grid(window: &AppWindow, ctx: &Context, binding: Binding<CardRow>) -> ModelRc<CardRow> {
    let weak = window.as_weak();
    let ctx = ctx.clone();
    ModelRc::new(CardGrid::new(move |request| {
        spawn_grid(weak.clone(), &ctx, binding, request)
    }))
}

fn spawn_tracks(
    window: slint::Weak<AppWindow>,
    ctx: &Context,
    binding: Binding<TrackItem>,
    request: Request,
) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    ctx.rt.spawn(async move {
        match request {
            Request::Count { generation, query } => {
                let result = server.song_count(&query).await;
                on_ui(&window, move |window| {
                    let total = result.as_ref().map_or(0, |total| *total);
                    if with_table(window, binding, |table| {
                        table.apply_count(generation, total)
                    }) == Some(true)
                    {
                        (binding.loading)(window, false);
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
                        with_table(window, binding, |table| {
                            table.apply_page(generation, page, rows)
                        });
                    }
                    Err(e) => {
                        with_table(window, binding, |table| table.page_failed(generation, page));
                        show_status(window, &format!("Could not load tracks: {e}"));
                    }
                });
            }
        }
    });
}

fn spawn_grid(
    window: slint::Weak<AppWindow>,
    ctx: &Context,
    binding: Binding<CardRow>,
    request: GridRequest,
) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    match request {
        GridRequest::Page {
            generation,
            source,
            page,
        } => {
            ctx.rt.spawn(async move {
                let paging = Paging {
                    start_index: page * GRID_PAGE,
                    limit: GRID_PAGE,
                };
                let result = source.fetch(server.as_ref(), paging).await;
                on_ui(&window, move |window| match result {
                    Ok((cards, total)) => {
                        if with_grid(window, binding, |grid| {
                            grid.apply_page(generation, page, cards, total)
                        }) == Some(true)
                        {
                            (binding.loading)(window, false);
                        }
                    }
                    Err(e) => {
                        if with_grid(window, binding, |grid| grid.page_failed(generation, page))
                            == Some(true)
                        {
                            (binding.loading)(window, false);
                            show_status(window, &format!("Could not load the list: {e}"));
                        }
                    }
                });
            });
        }
        GridRequest::Cover { cover_id } => {
            let covers = ctx.covers.clone();
            ctx.rt.spawn(async move {
                let result = covers.cover(server.as_ref(), &cover_id, COVER_SIZE).await;
                on_ui(&window, move |window| {
                    let image = result.ok().and_then(|cover| to_image(&cover));
                    with_grid(window, binding, |grid| grid.apply_cover(&cover_id, image));
                });
            });
        }
    }
}

/// Flips the favorite flag of a card right away and reverts it if the server refuses.
pub fn toggle_card(window: &AppWindow, ctx: &Context, binding: Binding<CardRow>, id: &str) {
    let Some((kind, favorite)) = with_grid(window, binding, |grid| {
        let favorite = grid.favorite(id)?;
        grid.set_favorite(id, !favorite);
        Some((grid.kind()?, favorite))
    })
    .flatten() else {
        return;
    };
    let Some(server) = ctx.session.get() else {
        return;
    };
    let weak = window.as_weak();
    let id = id.to_owned();
    ctx.rt.spawn(async move {
        if let Err(e) = server
            .set_favorite(kind, std::slice::from_ref(&id), !favorite)
            .await
        {
            on_ui(&weak, move |window| {
                with_grid(window, binding, |grid| grid.set_favorite(&id, favorite));
                show_status(window, &format!("Could not update favorite: {e}"));
            });
        }
    });
}

/// Flips the favorite flag of a track row right away and reverts it if the server refuses.
pub fn toggle_track(window: &AppWindow, ctx: &Context, binding: Binding<TrackItem>, index: usize) {
    let Some((id, favorite)) = with_table(window, binding, |table| {
        let (id, favorite) = table.favorite_at(index)?;
        table.set_favorite(index, &id, !favorite);
        Some((id, favorite))
    })
    .flatten() else {
        return;
    };
    let Some(server) = ctx.session.get() else {
        return;
    };
    let weak = window.as_weak();
    ctx.rt.spawn(async move {
        if let Err(e) = server
            .set_favorite(ItemKind::Song, std::slice::from_ref(&id), !favorite)
            .await
        {
            on_ui(&weak, move |window| {
                with_table(window, binding, |table| {
                    table.set_favorite(index, &id, favorite)
                });
                show_status(window, &format!("Could not update favorite: {e}"));
            });
        }
    });
}

/// Plays the table's whole server-side list starting at `index`.
pub fn play_track(window: &AppWindow, ctx: &Context, binding: Binding<TrackItem>, index: i32) {
    let Ok(index) = usize::try_from(index) else {
        return;
    };
    let target = with_table(window, binding, |table| {
        table
            .song_id_at(index)
            .map(|song_id| (table.query(), song_id))
    });
    if let Some(Some((query, song_id))) = target {
        ctx.send(Command::PlayFromList { query, song_id });
    }
}

pub fn genre_song_query(genre_id: &str) -> SongListQuery {
    SongListQuery {
        genre_ids: vec![genre_id.to_owned()],
        ..SongListQuery::default()
    }
}

pub fn genre_album_query(genre_id: &str) -> AlbumListQuery {
    AlbumListQuery {
        genre_ids: vec![genre_id.to_owned()],
        ..AlbumListQuery::default()
    }
}

fn count(value: Option<u32>) -> String {
    value.map(|n| n.to_string()).unwrap_or_default()
}

fn genre_row(genre: &Genre) -> GenreRow {
    GenreRow {
        id: genre.id.as_str().into(),
        name: genre.name.as_str().into(),
        albums: count(genre.album_count).into(),
        tracks: count(genre.song_count).into(),
    }
}

/// Genres whose name contains `filter`, ignoring case.
fn filter_genres(genres: &[Genre], filter: &str) -> Vec<GenreRow> {
    let filter = filter.trim().to_lowercase();
    genres
        .iter()
        .filter(|genre| filter.is_empty() || genre.name.to_lowercase().contains(&filter))
        .map(genre_row)
        .collect()
}

#[derive(Default)]
struct GenreList {
    genres: Vec<Genre>,
    filter: String,
    generation: u64,
}

const ALBUMS: Binding<CardRow> = Binding {
    rows: |window| window.global::<GenresState>().get_albums(),
    loading: |window, loading| window.global::<GenresState>().set_albums_loading(loading),
};

const TRACKS: Binding<TrackItem> = Binding {
    rows: |window| window.global::<GenresState>().get_tracks(),
    loading: |window, loading| window.global::<GenresState>().set_tracks_loading(loading),
};

fn show_genre(window: &AppWindow, list: &GenreList) {
    let state = window.global::<GenresState>();
    let id = window.global::<AppState>().get_detail_id();
    let genre = list.genres.iter().find(|genre| genre.id == id.as_str());
    state.set_title(genre.map(|g| g.name.as_str()).unwrap_or_default().into());
    state.set_album_count(count(genre.and_then(|g| g.album_count)).into());
    state.set_track_count(count(genre.and_then(|g| g.song_count)).into());
}

fn fetch_genres(window: &AppWindow, ctx: &Context, list: &Rc<RefCell<GenreList>>) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let generation = {
        let mut list = list.borrow_mut();
        list.generation += 1;
        list.generation
    };
    window.global::<GenresState>().set_genres_loading(true);
    let weak = window.as_weak();
    let list = Rc::clone(list);
    let (sender, receiver) = tokio::sync::oneshot::channel();
    ctx.rt.spawn(async move {
        let result = server.genre_list().await;
        let _ = sender.send(result);
    });
    let _ = slint::spawn_local(async move {
        let Ok(result) = receiver.await else {
            return;
        };
        let Some(window) = weak.upgrade() else {
            return;
        };
        let mut list = list.borrow_mut();
        if list.generation != generation {
            return;
        }
        let state = window.global::<GenresState>();
        state.set_genres_loading(false);
        match result {
            Ok(genres) => {
                list.genres = genres;
                state.set_genres(ModelRc::new(VecModel::from(filter_genres(
                    &list.genres,
                    &list.filter,
                ))));
                show_genre(&window, &list);
            }
            Err(e) => show_status(&window, &format!("Could not load genres: {e}")),
        }
    });
}

pub fn install(window: &AppWindow, ctx: &Context) {
    let state = window.global::<GenresState>();
    state.set_albums(card_grid(window, ctx, ALBUMS));
    state.set_tracks(track_table(window, ctx, false, TRACKS));
    let list: Rc<RefCell<GenreList>> = Rc::default();

    let weak = window.as_weak();
    let load_ctx = ctx.clone();
    let load_list = Rc::clone(&list);
    state.on_load(move |id| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let state = window.global::<GenresState>();
        if id.is_empty() {
            with_grid(&window, ALBUMS, |grid| grid.reset(None));
            load_list.borrow_mut().filter.clear();
            fetch_genres(&window, &load_ctx, &load_list);
            return;
        }
        state.set_albums_loading(true);
        state.set_tracks_loading(true);
        with_grid(&window, ALBUMS, |grid| {
            grid.reset(Some(CardSource::Albums(genre_album_query(&id))))
        });
        with_table(&window, TRACKS, |table| table.reset(genre_song_query(&id)));
        show_genre(&window, &load_list.borrow());
        if load_list
            .borrow()
            .genres
            .iter()
            .all(|genre| genre.id != id.as_str())
        {
            fetch_genres(&window, &load_ctx, &load_list);
        }
    });

    let weak = window.as_weak();
    state.on_filter_edited(move |text| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let mut list = list.borrow_mut();
        list.filter = text.to_string();
        window
            .global::<GenresState>()
            .set_genres(ModelRc::new(VecModel::from(filter_genres(
                &list.genres,
                &list.filter,
            ))));
    });

    let weak = window.as_weak();
    state.on_album_columns(move |columns| {
        if let Some(window) = weak.upgrade() {
            with_grid(&window, ALBUMS, |grid| {
                grid.set_columns(usize::try_from(columns).unwrap_or(1))
            });
        }
    });

    let weak = window.as_weak();
    let play_ctx = ctx.clone();
    state.on_play_track(move |index| {
        if let Some(window) = weak.upgrade() {
            play_track(&window, &play_ctx, TRACKS, index);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    type Log = Rc<RefCell<Vec<String>>>;

    fn grid(log: &Log) -> CardGrid {
        let sink = Rc::clone(log);
        CardGrid::new(move |request| {
            sink.borrow_mut().push(match request {
                GridRequest::Page {
                    generation, page, ..
                } => format!("page {generation}:{page}"),
                GridRequest::Cover { cover_id } => format!("cover {cover_id}"),
            })
        })
    }

    fn cards(range: std::ops::Range<usize>) -> Vec<CardData> {
        range
            .map(|i| CardData {
                id: format!("album-{i}"),
                title: format!("Album {i}"),
                subtitle: String::new(),
                cover_id: Some(format!("al-{i}")),
                favorite: true,
            })
            .collect()
    }

    fn drain(log: &Log) -> Vec<String> {
        log.borrow_mut().drain(..).collect()
    }

    fn genre(id: &str, name: &str) -> Genre {
        Genre {
            id: id.into(),
            name: name.into(),
            song_count: Some(12),
            album_count: Some(1),
        }
    }

    #[test]
    fn genres_filter_by_name_ignoring_case() {
        let genres = [
            genre("1", "Rock"),
            genre("2", "Post-Rock"),
            genre("3", "Jazz"),
        ];
        let names = |filter| -> Vec<String> {
            filter_genres(&genres, filter)
                .iter()
                .map(|row| row.name.to_string())
                .collect()
        };
        assert_eq!(names(""), ["Rock", "Post-Rock", "Jazz"]);
        assert_eq!(names(" rock "), ["Rock", "Post-Rock"]);
        assert!(names("metal").is_empty());
        assert_eq!(filter_genres(&genres, "jazz")[0].tracks, "12");
    }

    #[test]
    fn genre_queries_filter_by_genre_id() {
        assert_eq!(genre_song_query("g1").genre_ids, ["g1"]);
        assert_eq!(genre_album_query("g1").genre_ids, ["g1"]);
        assert_eq!(genre_song_query("g1").favorite, None);
    }

    #[test]
    fn grid_pages_rows_and_lazy_covers() {
        let log: Log = Rc::default();
        let grid = grid(&log);
        grid.set_columns(4);
        grid.reset(Some(CardSource::Albums(AlbumListQuery::default())));
        assert_eq!(drain(&log), ["page 1:0"]);
        assert_eq!(grid.row_count(), 0);

        assert!(grid.apply_page(1, 0, cards(0..GRID_PAGE), None));
        assert_eq!(grid.row_count(), (GRID_PAGE + 1).div_ceil(4));
        let row = grid.row_data(0).map(|row| row.cards).unwrap_or_default();
        assert_eq!(row.row_count(), 4);
        assert!(
            row.row_data(0)
                .is_some_and(|card| card.loaded && !card.has_cover)
        );
        assert_eq!(
            drain(&log),
            ["cover al-0", "cover al-1", "cover al-2", "cover al-3"]
        );
        grid.row_data(0);
        assert!(drain(&log).is_empty());

        grid.row_data(GRID_PAGE / 4);
        assert_eq!(drain(&log), ["page 1:1"]);
        assert!(grid.apply_page(1, 1, cards(GRID_PAGE..GRID_PAGE + 2), None));
        assert_eq!(grid.row_count(), (GRID_PAGE + 2).div_ceil(4));

        grid.apply_cover("al-1", Some(Image::default()));
        let row = grid.row_data(0).map(|row| row.cards).unwrap_or_default();
        assert!(row.row_data(1).is_some_and(|card| card.has_cover));

        grid.reset(None);
        assert!(!grid.apply_page(1, 2, cards(0..1), Some(1)));
        assert_eq!(grid.row_count(), 0);
        assert!(drain(&log).is_empty());
    }

    #[test]
    fn grid_total_and_columns_shape_the_rows() {
        let log: Log = Rc::default();
        let grid = grid(&log);
        grid.reset(Some(CardSource::Artists(ArtistListQuery::default())));
        assert!(grid.apply_page(1, 0, cards(0..10), Some(10)));
        assert_eq!(grid.row_count(), 10);
        grid.set_columns(3);
        assert_eq!(grid.row_count(), 4);
        assert_eq!(grid.row_data(3).map(|row| row.cards.row_count()), Some(1));
        assert!(grid.row_data(4).is_none());
        assert_eq!(grid.kind(), Some(ItemKind::Artist));
    }

    #[test]
    fn grid_favorites_flip_in_place() {
        let log: Log = Rc::default();
        let grid = grid(&log);
        grid.set_columns(2);
        grid.reset(Some(CardSource::Albums(AlbumListQuery::default())));
        grid.apply_page(1, 0, cards(0..3), Some(3));
        assert_eq!(grid.favorite("album-2"), Some(true));
        grid.set_favorite("album-2", false);
        assert_eq!(grid.favorite("album-2"), Some(false));
        let row = grid.row_data(1).map(|row| row.cards).unwrap_or_default();
        assert!(row.row_data(0).is_some_and(|card| !card.favorite));
        assert_eq!(grid.favorite("missing"), None);
    }

    #[test]
    fn track_table_favorites_flip_and_reset() {
        let table = TrackTable::new(true, |_| {});
        table.reset(SongListQuery::default());
        assert!(table.apply_count(1, 2));
        let songs = [
            crate::demo::test_song("a", 1000),
            crate::demo::test_song("b", 1000),
        ];
        table.apply_page(
            1,
            0,
            songs
                .iter()
                .enumerate()
                .map(|(i, s)| track_row(s, i))
                .collect(),
        );
        assert_eq!(table.favorite_at(1), Some(("b".to_owned(), true)));
        table.set_favorite(1, "b", false);
        assert!(table.row_data(1).is_some_and(|item| !item.favorite));
        assert!(table.row_data(0).is_some_and(|item| item.favorite));
        table.reset(SongListQuery::default());
        assert!(table.apply_count(2, 2));
        table.apply_page(
            2,
            0,
            songs
                .iter()
                .enumerate()
                .map(|(i, s)| track_row(s, i))
                .collect(),
        );
        assert_eq!(table.favorite_at(1), Some(("b".to_owned(), true)));
    }

    #[test]
    fn cover_cache_evicts_least_recently_used() {
        let mut covers = CoverLru::new(2);
        covers.insert("a".into(), Image::default());
        covers.insert("b".into(), Image::default());
        assert!(covers.get("a").is_some());
        covers.insert("c".into(), Image::default());
        assert!(covers.get("b").is_none());
        assert!(covers.get("a").is_some() && covers.get("c").is_some());
    }

    #[test]
    fn cards_map_albums_and_artists() {
        let album = Album {
            id: "al".into(),
            server_id: "s".into(),
            name: "Name".into(),
            album_artists: vec![feishin_core::model::ArtistRef {
                id: "ar".into(),
                name: "Ada".into(),
            }],
            artists: Vec::new(),
            year: None,
            genres: Vec::new(),
            song_count: 1,
            duration_ms: 0,
            cover_art_id: Some("c".into()),
            play_count: 0,
            user_rating: None,
            favorite: true,
            created_at: None,
            last_played_at: None,
        };
        let card = album_card(&album);
        assert_eq!(
            (
                card.subtitle.as_str(),
                card.cover_id.as_deref(),
                card.favorite
            ),
            ("Ada", Some("c"), true)
        );
        let artist = Artist {
            id: "ar".into(),
            server_id: "s".into(),
            name: "Ada".into(),
            album_count: Some(3),
            song_count: None,
            cover_art_id: None,
            favorite: false,
            user_rating: None,
            biography: None,
        };
        assert_eq!(artist_card(&artist).subtitle, "3 albums");
    }
}
