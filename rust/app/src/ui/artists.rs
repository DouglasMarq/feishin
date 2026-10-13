use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use std::time::Duration;

use feishin_core::model::{Artist, ArtistListQuery, ArtistSort, Paging, SortOrder};
use slint::{
    ComponentHandle, Image, Model, ModelNotify, ModelRc, ModelTracker, SharedString, Timer,
    TimerMode, VecModel,
};

use super::{AppWindow, ArtistCard, ArtistGridRow, ArtistsState, Context, on_ui};

pub const PAGE_SIZE: usize = 100;
/// Rows from the end of the loaded list at which the next page is requested.
const PREFETCH: usize = 40;
const COVER_SIZE: u32 = 200;
const COVER_CAPACITY: usize = 96;
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(300);

pub const SORTS: [(&str, ArtistSort); 7] = [
    ("Name", ArtistSort::Name),
    ("Album count", ArtistSort::AlbumCount),
    ("Song count", ArtistSort::SongCount),
    ("Play count", ArtistSort::PlayCount),
    ("Rating", ArtistSort::Rating),
    ("Favorited", ArtistSort::Favorited),
    ("Random", ArtistSort::Random),
];

/// Bounded set of decoded images keyed by row index, evicting the least recently used.
pub struct CoverLru {
    capacity: usize,
    images: HashMap<usize, Image>,
    order: VecDeque<usize>,
    pending: HashSet<usize>,
}

impl CoverLru {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            images: HashMap::new(),
            order: VecDeque::new(),
            pending: HashSet::new(),
        }
    }

    pub fn get(&mut self, index: usize) -> Option<Image> {
        let image = self.images.get(&index)?.clone();
        if let Some(pos) = self.order.iter().position(|&i| i == index) {
            self.order.remove(pos);
        }
        self.order.push_back(index);
        Some(image)
    }

    /// Marks `index` as requested; `false` when it is already loaded or in flight.
    pub fn begin(&mut self, index: usize) -> bool {
        !self.images.contains_key(&index) && self.pending.insert(index)
    }

    pub fn failed(&mut self, index: usize) {
        self.pending.remove(&index);
    }

    /// Stores an image and returns the indexes evicted to stay within capacity.
    pub fn insert(&mut self, index: usize, image: Image) -> Vec<usize> {
        self.pending.remove(&index);
        if self.images.insert(index, image).is_none() {
            self.order.push_back(index);
        }
        let mut evicted = Vec::new();
        while self.order.len() > self.capacity {
            if let Some(old) = self.order.pop_front() {
                self.images.remove(&old);
                evicted.push(old);
            }
        }
        evicted
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArtistItem {
    pub id: String,
    pub name: String,
    pub album_count: Option<u32>,
    pub song_count: Option<u32>,
    pub favorite: bool,
    pub cover_art_id: Option<String>,
}

impl From<Artist> for ArtistItem {
    fn from(artist: Artist) -> Self {
        Self {
            id: artist.id,
            name: artist.name,
            album_count: artist.album_count,
            song_count: artist.song_count,
            favorite: artist.favorite,
            cover_art_id: artist.cover_art_id,
        }
    }
}

pub fn count_label(count: Option<u32>, singular: &str, plural: &str) -> String {
    match count {
        Some(1) => format!("1 {singular}"),
        Some(n) => format!("{n} {plural}"),
        None => String::new(),
    }
}

pub enum Request {
    Page {
        generation: u64,
        query: ArtistListQuery,
        start: usize,
    },
    Cover {
        generation: u64,
        index: usize,
        cover_art_id: String,
    },
}

struct ListState {
    generation: u64,
    query: ArtistListQuery,
    items: Vec<ArtistItem>,
    fetching: bool,
    complete: bool,
    failed: bool,
    covers: CoverLru,
}

impl Default for ListState {
    fn default() -> Self {
        Self {
            generation: 0,
            query: ArtistListQuery::default(),
            items: Vec::new(),
            fetching: false,
            complete: false,
            failed: false,
            covers: CoverLru::new(COVER_CAPACITY),
        }
    }
}

/// Artists loaded so far, paged in as the user scrolls (the API has no count), shared by the list
/// and grid models. Covers are decoded only for rows Slint asks for and kept in a bounded LRU.
pub struct ArtistList {
    state: RefCell<ListState>,
    columns: Cell<usize>,
    list_notify: ModelNotify,
    grid_notify: ModelNotify,
    fetch: Box<dyn Fn(Request)>,
}

impl ArtistList {
    pub fn new(fetch: impl Fn(Request) + 'static) -> Self {
        Self {
            state: RefCell::default(),
            columns: Cell::new(1),
            list_notify: ModelNotify::default(),
            grid_notify: ModelNotify::default(),
            fetch: Box::new(fetch),
        }
    }

    pub fn generation(&self) -> u64 {
        self.state.borrow().generation
    }

    pub fn len(&self) -> usize {
        self.state.borrow().items.len()
    }

    pub fn is_complete(&self) -> bool {
        self.state.borrow().complete
    }

    pub fn reset(&self, query: ArtistListQuery) {
        let generation = {
            let mut state = self.state.borrow_mut();
            *state = ListState {
                generation: state.generation + 1,
                query: query.clone(),
                fetching: true,
                ..ListState::default()
            };
            state.generation
        };
        self.list_notify.reset();
        self.grid_notify.reset();
        (self.fetch)(Request::Page {
            generation,
            query,
            start: 0,
        });
    }

    pub fn set_columns(&self, columns: usize) {
        let columns = columns.max(1);
        if self.columns.replace(columns) != columns {
            self.grid_notify.reset();
        }
    }

    fn grid_rows(&self, len: usize) -> usize {
        len.div_ceil(self.columns.get())
    }

    /// Appends a page; returns `false` for stale or duplicate responses.
    pub fn apply_page(&self, generation: u64, start: usize, items: Vec<ArtistItem>) -> bool {
        let (old_len, new_len) = {
            let mut state = self.state.borrow_mut();
            if state.generation != generation || start != state.items.len() {
                return false;
            }
            state.fetching = false;
            state.complete = items.len() < PAGE_SIZE;
            let old_len = state.items.len();
            state.items.extend(items);
            (old_len, state.items.len())
        };
        if new_len > old_len {
            self.list_notify.row_added(old_len, new_len - old_len);
            let (old_rows, new_rows) = (self.grid_rows(old_len), self.grid_rows(new_len));
            if old_len % self.columns.get() != 0 {
                self.grid_notify.row_changed(old_rows - 1);
            }
            if new_rows > old_rows {
                self.grid_notify.row_added(old_rows, new_rows - old_rows);
            }
        }
        true
    }

    /// Stops paging after an error until the next reset.
    pub fn page_failed(&self, generation: u64) -> bool {
        let mut state = self.state.borrow_mut();
        if state.generation != generation {
            return false;
        }
        state.fetching = false;
        state.failed = true;
        true
    }

    pub fn apply_cover(&self, generation: u64, index: usize, image: Option<Image>) {
        {
            let mut state = self.state.borrow_mut();
            if state.generation != generation {
                return;
            }
            match image {
                Some(image) => {
                    state.covers.insert(index, image);
                }
                None => state.covers.failed(index),
            }
        }
        self.list_notify.row_changed(index);
        self.grid_notify.row_changed(index / self.columns.get());
    }

    pub fn set_favorite(&self, id: &str, favorite: bool) {
        let index = {
            let mut state = self.state.borrow_mut();
            let Some(index) = state.items.iter().position(|item| item.id == id) else {
                return;
            };
            state.items[index].favorite = favorite;
            index
        };
        self.list_notify.row_changed(index);
        self.grid_notify.row_changed(index / self.columns.get());
    }

    fn card(&self, index: usize) -> Option<ArtistCard> {
        let mut requests = Vec::new();
        let card = {
            let mut state = self.state.borrow_mut();
            let state = &mut *state;
            let item = state.items.get(index)?.clone();
            if !state.complete
                && !state.fetching
                && !state.failed
                && index + PREFETCH >= state.items.len()
            {
                state.fetching = true;
                requests.push(Request::Page {
                    generation: state.generation,
                    query: state.query.clone(),
                    start: state.items.len(),
                });
            }
            let image = state.covers.get(index);
            if image.is_none()
                && let Some(cover_art_id) = item.cover_art_id.clone()
                && state.covers.begin(index)
            {
                requests.push(Request::Cover {
                    generation: state.generation,
                    index,
                    cover_art_id,
                });
            }
            ArtistCard {
                id: item.id.into(),
                name: item.name.into(),
                albums: count_label(item.album_count, "album", "albums").into(),
                songs: count_label(item.song_count, "song", "songs").into(),
                favorite: item.favorite,
                has_image: image.is_some(),
                image: image.unwrap_or_default(),
            }
        };
        for request in requests {
            (self.fetch)(request);
        }
        Some(card)
    }
}

pub struct ListModel(pub Rc<ArtistList>);

impl Model for ListModel {
    type Data = ArtistCard;

    fn row_count(&self) -> usize {
        self.0.len()
    }

    fn row_data(&self, row: usize) -> Option<ArtistCard> {
        self.0.card(row)
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.0.list_notify
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub struct GridModel(pub Rc<ArtistList>);

impl Model for GridModel {
    type Data = ArtistGridRow;

    fn row_count(&self) -> usize {
        self.0.grid_rows(self.0.len())
    }

    fn row_data(&self, row: usize) -> Option<ArtistGridRow> {
        let columns = self.0.columns.get();
        let start = row * columns;
        let end = (start + columns).min(self.0.len());
        if start >= end {
            return None;
        }
        let cards: Vec<ArtistCard> = (start..end).filter_map(|i| self.0.card(i)).collect();
        Some(ArtistGridRow {
            cards: ModelRc::new(VecModel::from(cards)),
        })
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.0.grid_notify
    }
}

pub fn build_query(
    sort_index: i32,
    descending: bool,
    favorites_only: bool,
    search: &str,
) -> ArtistListQuery {
    let sort = usize::try_from(sort_index)
        .ok()
        .and_then(|i| SORTS.get(i))
        .map_or(ArtistSort::Name, |(_, sort)| *sort);
    let search = search.trim();
    ArtistListQuery {
        sort,
        order: if descending {
            SortOrder::Desc
        } else {
            SortOrder::Asc
        },
        search: (!search.is_empty()).then(|| search.to_owned()),
        favorite: favorites_only.then_some(true),
        music_folder_id: None,
    }
}

fn with_list<R>(window: &AppWindow, f: impl FnOnce(&ArtistList) -> R) -> Option<R> {
    let items = window.global::<ArtistsState>().get_items();
    items.as_any().downcast_ref::<ListModel>().map(|m| f(&m.0))
}

/// Updates the favorite flag of a loaded artist, e.g. after toggling it on the detail page.
pub fn set_favorite(window: &AppWindow, id: &str, favorite: bool) {
    with_list(window, |list| list.set_favorite(id, favorite));
}

fn finish_loading(window: &AppWindow, list: &ArtistList) {
    let state = window.global::<ArtistsState>();
    state.set_loading(false);
    state.set_complete(list.is_complete());
}

fn spawn_request(window: slint::Weak<AppWindow>, ctx: &Context, request: Request) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let covers = ctx.covers.clone();
    ctx.rt.spawn(async move {
        match request {
            Request::Page {
                generation,
                query,
                start,
            } => {
                let paging = Paging {
                    start_index: start,
                    limit: PAGE_SIZE,
                };
                let result = server
                    .artist_list(&query, paging)
                    .await
                    .map(|page| page.items.into_iter().map(ArtistItem::from).collect());
                on_ui(&window, move |window| {
                    let Some(list) = window
                        .global::<ArtistsState>()
                        .get_items()
                        .as_any()
                        .downcast_ref::<ListModel>()
                        .map(|m| Rc::clone(&m.0))
                    else {
                        return;
                    };
                    match result {
                        Ok(items) => {
                            if list.apply_page(generation, start, items) {
                                finish_loading(window, &list);
                            }
                        }
                        Err(e) => {
                            if list.page_failed(generation) {
                                finish_loading(window, &list);
                                super::show_status(window, &format!("Could not load artists: {e}"));
                            }
                        }
                    }
                });
            }
            Request::Cover {
                generation,
                index,
                cover_art_id,
            } => {
                let cover = covers
                    .cover(server.as_ref(), &cover_art_id, COVER_SIZE)
                    .await;
                if let Err(e) = &cover {
                    tracing::debug!("artist cover {cover_art_id}: {e:#}");
                }
                on_ui(&window, move |window| {
                    let image = cover.ok().and_then(|cover| super::player::to_image(&cover));
                    with_list(window, |list| list.apply_cover(generation, index, image));
                });
            }
        }
    });
}

fn reload(window: &AppWindow, search: &str) {
    let state = window.global::<ArtistsState>();
    let query = build_query(
        state.get_sort_index(),
        state.get_descending(),
        state.get_favorites_only(),
        search,
    );
    state.set_loading(true);
    state.set_complete(false);
    with_list(window, |list| list.reset(query));
}

pub fn install(window: &AppWindow, ctx: &Context) {
    let state = window.global::<ArtistsState>();
    let weak = window.as_weak();
    let fetch_ctx = ctx.clone();
    let list = Rc::new(ArtistList::new(move |request| {
        spawn_request(weak.clone(), &fetch_ctx, request);
    }));
    state.set_items(ModelRc::new(ListModel(Rc::clone(&list))));
    state.set_grid_rows(ModelRc::new(GridModel(Rc::clone(&list))));
    let options: Vec<SharedString> = SORTS.iter().map(|(label, _)| (*label).into()).collect();
    state.set_sort_options(ModelRc::new(VecModel::from(options)));

    let search: Rc<RefCell<String>> = Rc::default();
    let loaded_for: Rc<RefCell<Option<String>>> = Rc::default();

    let weak = window.as_weak();
    let session = ctx.session.clone();
    let current_search = Rc::clone(&search);
    state.on_load(move |_| {
        let (Some(window), Some(server)) = (weak.upgrade(), session.get()) else {
            return;
        };
        let server_id = server.config().id.clone();
        let mut loaded_for = loaded_for.borrow_mut();
        if loaded_for.as_deref() != Some(server_id.as_str()) || list.generation() == 0 {
            *loaded_for = Some(server_id);
            reload(&window, &current_search.borrow());
        }
    });

    let weak = window.as_weak();
    let current_search = Rc::clone(&search);
    state.on_query_changed(move || {
        if let Some(window) = weak.upgrade() {
            reload(&window, &current_search.borrow());
        }
    });

    let weak = window.as_weak();
    let debounce = Rc::new(Timer::default());
    state.on_search_edited(move |text| {
        let weak = weak.clone();
        let search = Rc::clone(&search);
        debounce.start(TimerMode::SingleShot, SEARCH_DEBOUNCE, move || {
            let Some(window) = weak.upgrade() else {
                return;
            };
            *search.borrow_mut() = text.trim().to_owned();
            reload(&window, &search.borrow());
        });
    });

    let weak = window.as_weak();
    state.on_set_columns(move |columns| {
        if let Some(window) = weak.upgrade() {
            with_list(&window, |list| {
                list.set_columns(usize::try_from(columns).unwrap_or(1));
            });
        }
    });

    let weak = window.as_weak();
    let play_ctx = ctx.clone();
    state.on_play(move |id| {
        if let Some(window) = weak.upgrade() {
            super::artist_detail::play_artist(&window, &play_ctx, id.as_str(), false);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    type Log = Rc<RefCell<Vec<String>>>;

    fn list() -> (ArtistList, Log) {
        let log: Log = Rc::default();
        let sink = Rc::clone(&log);
        let list = ArtistList::new(move |request| {
            sink.borrow_mut().push(match request {
                Request::Page {
                    generation, start, ..
                } => format!("page {generation} {start}"),
                Request::Cover {
                    generation, index, ..
                } => format!("cover {generation} {index}"),
            });
        });
        (list, log)
    }

    fn items(start: usize, count: usize) -> Vec<ArtistItem> {
        (start..start + count)
            .map(|i| ArtistItem {
                id: format!("artist-{i}"),
                name: format!("Artist {i}"),
                album_count: Some(2),
                song_count: Some(1),
                favorite: false,
                cover_art_id: (i % 2 == 0).then(|| format!("ar-{i}")),
            })
            .collect()
    }

    fn drain(log: &Log) -> Vec<String> {
        log.borrow_mut().drain(..).collect()
    }

    #[test]
    fn pages_until_a_short_page() {
        let (list, log) = list();
        list.reset(ArtistListQuery::default());
        assert_eq!(drain(&log), ["page 1 0"]);

        assert!(list.apply_page(1, 0, items(0, PAGE_SIZE)));
        assert!(!list.is_complete());
        list.card(10);
        assert_eq!(drain(&log), ["cover 1 10"]);

        list.card(PAGE_SIZE - 1);
        list.card(PAGE_SIZE - 2);
        assert_eq!(
            drain(&log),
            [
                format!("page 1 {PAGE_SIZE}"),
                format!("cover 1 {}", PAGE_SIZE - 2)
            ]
        );
        assert!(!list.apply_page(1, 0, items(0, PAGE_SIZE)));
        assert!(list.apply_page(1, PAGE_SIZE, items(PAGE_SIZE, 5)));
        assert!(list.is_complete());
        assert_eq!(list.len(), PAGE_SIZE + 5);
        list.card(PAGE_SIZE + 4);
        assert_eq!(drain(&log), [format!("cover 1 {}", PAGE_SIZE + 4)]);
    }

    #[test]
    fn stale_generations_are_ignored() {
        let (list, log) = list();
        list.reset(ArtistListQuery::default());
        list.reset(ArtistListQuery::default());
        assert_eq!(drain(&log), ["page 1 0", "page 2 0"]);
        assert!(!list.apply_page(1, 0, items(0, 3)));
        assert!(!list.page_failed(1));
        assert_eq!(list.len(), 0);
        assert!(list.apply_page(2, 0, items(0, 3)));
        list.apply_cover(1, 0, Some(Image::default()));
        assert!(list.card(0).is_some_and(|card| !card.has_image));
    }

    #[test]
    fn failed_page_stops_paging_until_reset() {
        let (list, log) = list();
        list.reset(ArtistListQuery::default());
        assert!(list.apply_page(1, 0, items(1, PAGE_SIZE)));
        list.card(PAGE_SIZE - 1);
        drain(&log);
        assert!(list.page_failed(1));
        list.card(PAGE_SIZE - 1);
        assert!(drain(&log).is_empty());
    }

    #[test]
    fn grid_groups_rows_by_columns() {
        let (list, _log) = list();
        list.reset(ArtistListQuery::default());
        list.set_columns(4);
        assert!(list.apply_page(1, 0, items(0, 10)));
        let grid = GridModel(Rc::new(list));
        assert_eq!(grid.row_count(), 3);
        assert_eq!(grid.row_data(2).map(|row| row.cards.row_count()), Some(2));
        assert!(grid.row_data(3).is_none());
        grid.0.set_columns(5);
        assert_eq!(grid.row_count(), 2);
    }

    #[test]
    fn covers_are_cached_and_bounded() {
        let (list, log) = list();
        list.reset(ArtistListQuery::default());
        assert!(list.apply_page(1, 0, items(0, 4)));
        drain(&log);
        list.card(0);
        list.card(0);
        assert_eq!(drain(&log), ["cover 1 0"]);
        list.apply_cover(1, 0, Some(Image::default()));
        assert!(list.card(0).is_some_and(|card| card.has_image));
        assert!(drain(&log).is_empty());

        let mut lru = CoverLru::new(2);
        for i in 0..3 {
            assert!(lru.begin(i));
        }
        lru.insert(0, Image::default());
        lru.insert(1, Image::default());
        lru.get(0);
        assert_eq!(lru.insert(2, Image::default()), [1]);
        assert!(lru.get(1).is_none() && lru.get(2).is_some());
        assert!(lru.begin(1));
        assert!(!lru.begin(0));
    }

    #[test]
    fn favorites_update_loaded_items() {
        let (list, _log) = list();
        list.reset(ArtistListQuery::default());
        assert!(list.apply_page(1, 0, items(0, 3)));
        list.set_favorite("artist-1", true);
        assert!(list.card(1).is_some_and(|card| card.favorite));
    }

    #[test]
    fn query_follows_the_controls() {
        let query = build_query(2, true, true, "  vale ");
        assert_eq!(query.sort, ArtistSort::SongCount);
        assert_eq!(query.order, SortOrder::Desc);
        assert_eq!(query.search.as_deref(), Some("vale"));
        assert_eq!(query.favorite, Some(true));
        let query = build_query(-1, false, false, " ");
        assert_eq!(query, ArtistListQuery::default());
        assert_eq!(build_query(99, false, false, "").sort, ArtistSort::Name);
    }

    #[test]
    fn count_labels() {
        assert_eq!(count_label(Some(1), "album", "albums"), "1 album");
        assert_eq!(count_label(Some(3), "album", "albums"), "3 albums");
        assert_eq!(count_label(None, "album", "albums"), "");
    }
}
