use std::cell::RefCell;
use std::collections::BTreeSet;

use feishin_core::model::AlbumListQuery;
use slint::{Image, Model, ModelNotify, ModelRc, ModelTracker, VecModel};

use super::super::page_cache::{PageCache, PageShape};
use super::super::{AlbumGridCard, AlbumGridRow};
use super::card::CardData;
use super::cover_cache::CoverLru;

pub const PAGE_SIZE: usize = 60;
const COVER_CAPACITY: usize = 96;
const CACHED_PAGES: usize = 16;
const GRID_PAGES: PageShape = PageShape {
    page_size: PAGE_SIZE,
    capacity: CACHED_PAGES,
};

#[derive(Debug, Clone, PartialEq)]
pub enum GridRequest {
    Count {
        generation: u64,
        query: AlbumListQuery,
    },
    Page {
        generation: u64,
        query: AlbumListQuery,
        page: usize,
    },
    Cover {
        cover_art_id: String,
    },
}

struct GridState {
    generation: u64,
    query: Option<AlbumListQuery>,
    total: usize,
    columns: usize,
    pages: PageCache<CardData>,
    covers: CoverLru<Image>,
}

impl GridState {
    fn card(&self, index: usize) -> Option<&CardData> {
        self.pages.get(index)
    }

    fn rows_for(&self, indexes: impl IntoIterator<Item = usize>) -> BTreeSet<usize> {
        indexes
            .into_iter()
            .map(|index| index / self.columns)
            .collect()
    }

    fn indexes_with_cover(&self, cover_art_id: &str) -> Vec<usize> {
        self.pages
            .rows()
            .filter(|(_, card)| card.cover_art_id.as_deref() == Some(cover_art_id))
            .map(|(index, _)| index)
            .collect()
    }

    fn request_pages_around(&mut self, index: usize, requests: &mut Vec<GridRequest>) {
        let Some(query) = self.query.as_ref() else {
            return;
        };
        let generation = self.generation;
        let pages = self.pages.pages_to_fetch(index, self.total);
        requests.extend(pages.into_iter().map(|page| GridRequest::Page {
            generation,
            query: query.clone(),
            page,
        }));
    }

    fn cached_cover(&mut self, card: &CardData, requests: &mut Vec<GridRequest>) -> Option<Image> {
        let cover_art_id = card.cover_art_id.as_deref()?;
        let cached = self.covers.get(cover_art_id);
        if cached.is_none() && self.covers.begin(cover_art_id) {
            requests.push(GridRequest::Cover {
                cover_art_id: cover_art_id.to_owned(),
            });
        }
        cached
    }

    fn grid_card(&mut self, index: usize, requests: &mut Vec<GridRequest>) -> AlbumGridCard {
        self.request_pages_around(index, requests);
        let Some(data) = self.card(index).cloned() else {
            return AlbumGridCard::default();
        };
        let cover = self.cached_cover(&data, requests);
        AlbumGridCard {
            id: data.id,
            name: data.name,
            artists: data.artists,
            artist_id: data.artist_id,
            year: data.year,
            has_cover: cover.is_some(),
            cover: cover.unwrap_or_default(),
            loaded: true,
        }
    }

    fn row_cards(
        &mut self,
        row: usize,
        requests: &mut Vec<GridRequest>,
    ) -> Option<Vec<AlbumGridCard>> {
        let start = row * self.columns;
        if start >= self.total {
            return None;
        }
        let end = (start + self.columns).min(self.total);
        Some(
            (start..end)
                .map(|index| self.grid_card(index, requests))
                .collect(),
        )
    }
}

pub struct AlbumGridModel {
    state: RefCell<GridState>,
    notify: ModelNotify,
    fetch: Box<dyn Fn(GridRequest)>,
}

impl AlbumGridModel {
    pub fn new(fetch: impl Fn(GridRequest) + 'static) -> Self {
        Self {
            state: RefCell::new(GridState {
                generation: 0,
                query: None,
                total: 0,
                columns: 1,
                pages: PageCache::new(GRID_PAGES),
                covers: CoverLru::new(COVER_CAPACITY),
            }),
            notify: ModelNotify::default(),
            fetch: Box::new(fetch),
        }
    }

    pub fn reset(&self, query: AlbumListQuery) {
        let generation = {
            let mut state = self.state.borrow_mut();
            state.generation += 1;
            state.query = Some(query.clone());
            state.total = 0;
            state.pages = PageCache::new(GRID_PAGES);
            state.generation
        };
        self.notify.reset();
        (self.fetch)(GridRequest::Count { generation, query });
    }

    pub fn set_columns(&self, columns: usize) {
        {
            let mut state = self.state.borrow_mut();
            let columns = columns.max(1);
            if state.columns == columns {
                return;
            }
            state.columns = columns;
        }
        self.notify.reset();
    }

    pub fn clear_covers(&self) {
        self.state.borrow_mut().covers.clear();
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

    fn notify_rows_changed(&self, rows: BTreeSet<usize>) {
        for row in rows {
            self.notify.row_changed(row);
        }
    }

    pub fn apply_page(&self, generation: u64, page: usize, cards: Vec<CardData>) {
        let rows = {
            let mut state = self.state.borrow_mut();
            if state.generation != generation {
                return;
            }
            let start = state.pages.first_index_of(page);
            let end = (start + cards.len()).min(state.total);
            state.pages.insert(page, cards);
            state.rows_for(start..end)
        };
        self.notify_rows_changed(rows);
    }

    pub fn page_failed(&self, generation: u64, page: usize) {
        let mut state = self.state.borrow_mut();
        if state.generation == generation {
            state.pages.forget_request(page);
        }
    }

    pub fn apply_cover(&self, cover_art_id: &str, image: Option<Image>) {
        let rows = {
            let mut state = self.state.borrow_mut();
            let loaded = image.is_some();
            state.covers.finish(cover_art_id, image);
            if !loaded {
                return;
            }
            let indexes = state.indexes_with_cover(cover_art_id);
            state.rows_for(indexes)
        };
        self.notify_rows_changed(rows);
    }
}

impl Model for AlbumGridModel {
    type Data = AlbumGridRow;

    fn row_count(&self) -> usize {
        let state = self.state.borrow();
        state.total.div_ceil(state.columns)
    }

    fn row_data(&self, row: usize) -> Option<AlbumGridRow> {
        let mut requests = Vec::new();
        let cards = self.state.borrow_mut().row_cards(row, &mut requests)?;
        for request in requests {
            (self.fetch)(request);
        }
        Some(AlbumGridRow {
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

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::super::card::tests::album;
    use super::*;

    type RequestLog = Rc<RefCell<Vec<GridRequest>>>;

    fn recording() -> (RequestLog, AlbumGridModel) {
        let log: RequestLog = Rc::default();
        let sink = Rc::clone(&log);
        let model = AlbumGridModel::new(move |request| sink.borrow_mut().push(request));
        (log, model)
    }

    fn drain(log: &RequestLog) -> Vec<GridRequest> {
        log.borrow_mut().drain(..).collect()
    }

    fn counted_model_with_four_columns() -> (RequestLog, AlbumGridModel) {
        let (log, model) = recording();
        model.set_columns(4);
        model.reset(AlbumListQuery::default());
        assert!(matches!(
            drain(&log).as_slice(),
            [GridRequest::Count { generation: 1, .. }]
        ));
        assert!(model.apply_count(1, 130));
        assert_eq!(model.row_count(), 33);
        (log, model)
    }

    fn assert_missing_page_is_requested_once(log: &RequestLog, model: &AlbumGridModel) {
        let row = model.row_data(15).expect("row");
        assert_eq!(row.cards.row_count(), 4);
        assert!(row.cards.iter().all(|card| !card.loaded));
        model.row_data(16);
        assert!(matches!(
            drain(log).as_slice(),
            [
                GridRequest::Page {
                    generation: 1,
                    page: 1,
                    ..
                },
                GridRequest::Page {
                    generation: 1,
                    page: 2,
                    ..
                }
            ]
        ));
    }

    fn assert_loaded_page_requests_covers_once(log: &RequestLog, model: &AlbumGridModel) {
        let cards = (60..120)
            .map(|index| CardData::from_album(&album(index)))
            .collect();
        model.apply_page(1, 1, cards);
        let row = model.row_data(15).expect("row");
        assert_eq!(
            row.cards.row_data(0).map(|card| card.id),
            Some("album-60".into())
        );
        assert!(row.cards.iter().all(|card| card.loaded && !card.has_cover));
        let covers = drain(log)
            .iter()
            .filter(|request| matches!(request, GridRequest::Cover { .. }))
            .count();
        assert_eq!(covers, 4);
        model.row_data(15);
        assert!(log.borrow().is_empty());
    }

    #[test]
    fn rows_regroup_by_columns_and_request_pages_and_covers_once() {
        let (log, model) = counted_model_with_four_columns();
        assert_missing_page_is_requested_once(&log, &model);
        assert_loaded_page_requests_covers_once(&log, &model);

        model.apply_cover("al-61", Some(Image::default()));
        let row = model.row_data(15).expect("row");
        assert_eq!(row.cards.row_data(1).map(|card| card.has_cover), Some(true));

        model.set_columns(10);
        assert_eq!(model.row_count(), 13);
        assert_eq!(model.row_data(13), None);

        model.reset(AlbumListQuery::default());
        model.apply_page(1, 0, vec![CardData::default()]);
        assert!(!model.apply_count(1, 5));
        assert_eq!(model.row_count(), 0);
    }

    fn requested_pages(log: &RequestLog) -> Vec<usize> {
        drain(log)
            .into_iter()
            .filter_map(|request| match request {
                GridRequest::Page { page, .. } => Some(page),
                _ => None,
            })
            .collect()
    }

    fn album_page(page: usize) -> Vec<CardData> {
        (page * PAGE_SIZE..(page + 1) * PAGE_SIZE)
            .map(|index| CardData::from_album(&album(index)))
            .collect()
    }

    #[test]
    fn far_pages_are_evicted_and_fetched_again() {
        let (log, model) = recording();
        model.set_columns(PAGE_SIZE);
        model.reset(AlbumListQuery::default());
        assert!(model.apply_count(1, PAGE_SIZE * 40));
        drain(&log);
        for row in 0..40 {
            model.row_data(row);
            for page in requested_pages(&log) {
                model.apply_page(1, page, album_page(page));
            }
        }
        let first = model.row_data(0).expect("row");
        assert!(first.cards.iter().all(|card| !card.loaded));
        assert_eq!(requested_pages(&log), [0]);
        let kept = model.row_data(39 - CACHED_PAGES + 1).expect("row");
        assert!(kept.cards.iter().all(|card| card.loaded));
        let evicted = model.row_data(39 - CACHED_PAGES).expect("row");
        assert!(evicted.cards.iter().all(|card| !card.loaded));
    }
}
