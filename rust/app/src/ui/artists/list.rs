use std::cell::{Cell, RefCell};

use feishin_core::model::ArtistListQuery;
use slint::{Image, ModelNotify};

use super::super::ArtistCard;
use super::cover_lru::CoverLru;
use super::item::ArtistItem;

pub const PAGE_SIZE: usize = 100;
const PREFETCH_ROWS: usize = 40;
const COVER_CAPACITY: usize = 96;

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

impl ListState {
    fn needs_next_page_near(&self, index: usize) -> bool {
        !self.complete
            && !self.fetching
            && !self.failed
            && index + PREFETCH_ROWS >= self.items.len()
    }

    fn request_next_page_near(&mut self, index: usize, requests: &mut Vec<Request>) {
        if !self.needs_next_page_near(index) {
            return;
        }
        self.fetching = true;
        requests.push(Request::Page {
            generation: self.generation,
            query: self.query.clone(),
            start: self.items.len(),
        });
    }

    fn cached_cover(
        &mut self,
        index: usize,
        cover_art_id: Option<String>,
        requests: &mut Vec<Request>,
    ) -> Option<Image> {
        let image = self.covers.get(index);
        if image.is_none()
            && let Some(cover_art_id) = cover_art_id
            && self.covers.begin(index)
        {
            requests.push(Request::Cover {
                generation: self.generation,
                index,
                cover_art_id,
            });
        }
        image
    }

    fn card(&mut self, index: usize, requests: &mut Vec<Request>) -> Option<ArtistCard> {
        let item = self.items.get(index)?.clone();
        self.request_next_page_near(index, requests);
        let image = self.cached_cover(index, item.cover_art_id.clone(), requests);
        Some(item.into_card(image))
    }
}

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

    pub fn columns(&self) -> usize {
        self.columns.get()
    }

    pub fn list_notify(&self) -> &ModelNotify {
        &self.list_notify
    }

    pub fn grid_notify(&self) -> &ModelNotify {
        &self.grid_notify
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

    pub fn grid_rows(&self, len: usize) -> usize {
        len.div_ceil(self.columns.get())
    }

    fn notify_appended(&self, old_len: usize, new_len: usize) {
        self.list_notify.row_added(old_len, new_len - old_len);
        let (old_rows, new_rows) = (self.grid_rows(old_len), self.grid_rows(new_len));
        if !old_len.is_multiple_of(self.columns.get()) {
            self.grid_notify.row_changed(old_rows - 1);
        }
        if new_rows > old_rows {
            self.grid_notify.row_added(old_rows, new_rows - old_rows);
        }
    }

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
            self.notify_appended(old_len, new_len);
        }
        true
    }

    pub fn page_failed(&self, generation: u64) -> bool {
        let mut state = self.state.borrow_mut();
        if state.generation != generation {
            return false;
        }
        state.fetching = false;
        state.failed = true;
        true
    }

    fn notify_index_changed(&self, index: usize) {
        self.list_notify.row_changed(index);
        self.grid_notify.row_changed(index / self.columns.get());
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
        self.notify_index_changed(index);
    }

    pub fn set_favorite(&self, artist_id: &str, favorite: bool) {
        let index = {
            let mut state = self.state.borrow_mut();
            let Some((index, item)) = state
                .items
                .iter_mut()
                .enumerate()
                .find(|(_, item)| item.id == artist_id)
            else {
                return;
            };
            item.favorite = favorite;
            index
        };
        self.notify_index_changed(index);
    }

    pub fn card(&self, index: usize) -> Option<ArtistCard> {
        let mut requests = Vec::new();
        let card = self.state.borrow_mut().card(index, &mut requests)?;
        for request in requests {
            (self.fetch)(request);
        }
        Some(card)
    }
}

#[cfg(test)]
pub mod tests {
    use std::rc::Rc;

    use super::*;

    pub type RequestLog = Rc<RefCell<Vec<String>>>;

    fn describe(request: &Request) -> String {
        match request {
            Request::Page {
                generation, start, ..
            } => format!("page {generation} {start}"),
            Request::Cover {
                generation, index, ..
            } => format!("cover {generation} {index}"),
        }
    }

    pub fn list() -> (ArtistList, RequestLog) {
        let log: RequestLog = Rc::default();
        let sink = Rc::clone(&log);
        let list = ArtistList::new(move |request| sink.borrow_mut().push(describe(&request)));
        (list, log)
    }

    pub fn items(start: usize, count: usize) -> Vec<ArtistItem> {
        (start..start + count)
            .map(|index| ArtistItem {
                id: format!("artist-{index}"),
                name: format!("Artist {index}"),
                album_count: Some(2),
                song_count: Some(1),
                favorite: false,
                cover_art_id: (index % 2 == 0).then(|| format!("ar-{index}")),
            })
            .collect()
    }

    fn drain(log: &RequestLog) -> Vec<String> {
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
    fn covers_are_cached() {
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
    }

    #[test]
    fn favorites_update_loaded_items() {
        let (list, _log) = list();
        list.reset(ArtistListQuery::default());
        assert!(list.apply_page(1, 0, items(0, 3)));
        list.set_favorite("artist-1", true);
        assert!(list.card(1).is_some_and(|card| card.favorite));
    }
}
