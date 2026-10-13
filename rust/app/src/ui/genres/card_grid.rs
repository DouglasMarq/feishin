use std::cell::RefCell;
use std::cmp::Ordering;

use feishin_core::model::ItemKind;
use slint::{Image, Model, ModelNotify, ModelRc, ModelTracker, VecModel};

use super::card::{CardData, CardSource};
use super::cover_cache::remember_cover;
use super::grid_state::{GridRequest, GridState, RowChange};
use crate::ui::CardRow;

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

    pub fn reset(&self, source: Option<CardSource>) {
        let request = {
            let mut state = self.state.borrow_mut();
            *state = state.restarted(source);
            state.request_first_page()
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
        let columns = columns.max(1);
        {
            let mut state = self.state.borrow_mut();
            if state.columns == columns {
                return;
            }
            state.columns = columns;
        }
        self.notify.reset();
    }

    pub fn apply_page(
        &self,
        generation: u64,
        page: usize,
        cards: Vec<CardData>,
        total: Option<usize>,
    ) -> bool {
        let change = {
            let mut state = self.state.borrow_mut();
            if state.generation != generation {
                return false;
            }
            state.store_page(page, cards, total)
        };
        self.notify_row_change(change);
        true
    }

    fn notify_row_change(&self, change: RowChange) {
        let RowChange {
            old_rows,
            new_rows,
            changed,
        } = change;
        match new_rows.cmp(&old_rows) {
            Ordering::Greater => self.notify.row_added(old_rows, new_rows - old_rows),
            Ordering::Less => self.notify.row_removed(new_rows, old_rows - new_rows),
            Ordering::Equal => {}
        }
        for row in changed.start..changed.end.min(old_rows.min(new_rows)) {
            self.notify.row_changed(row);
        }
    }

    pub fn page_failed(&self, generation: u64, page: usize) -> bool {
        self.state
            .borrow_mut()
            .forget_page_request(generation, page)
    }

    pub fn apply_cover(&self, cover_id: &str, image: Option<Image>) {
        let Some(image) = image else {
            return;
        };
        remember_cover(cover_id.to_owned(), image);
        let rows = self.state.borrow_mut().cover_loaded(cover_id);
        for row in rows {
            self.notify.row_changed(row);
        }
    }

    pub fn favorite(&self, id: &str) -> Option<bool> {
        self.state.borrow().favorite(id)
    }

    pub fn set_favorite(&self, id: &str, favorite: bool) {
        let row = self.state.borrow_mut().override_favorite(id, favorite);
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
        let cards = self.state.borrow_mut().cards_for_row(row, &mut requests)?;
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

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use feishin_core::model::{AlbumListQuery, ArtistListQuery};

    use super::*;
    use crate::ui::genres::grid_state::GRID_PAGE;

    type RequestLog = Rc<RefCell<Vec<String>>>;

    fn logging_grid(log: &RequestLog) -> CardGrid {
        let sink = Rc::clone(log);
        CardGrid::new(move |request| {
            sink.borrow_mut().push(match request {
                GridRequest::Page {
                    generation, page, ..
                } => format!("page {generation}:{page}"),
                GridRequest::Cover { cover_id } => format!("cover {cover_id}"),
            });
        })
    }

    fn cards(range: std::ops::Range<usize>) -> Vec<CardData> {
        range
            .map(|number| CardData {
                id: format!("album-{number}"),
                title: format!("Album {number}"),
                subtitle: String::new(),
                cover_id: Some(format!("al-{number}")),
                favorite: true,
            })
            .collect()
    }

    fn drain(log: &RequestLog) -> Vec<String> {
        log.borrow_mut().drain(..).collect()
    }

    #[test]
    fn grid_pages_rows_and_lazy_covers() {
        let log: RequestLog = Rc::default();
        let grid = logging_grid(&log);
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
        let log: RequestLog = Rc::default();
        let grid = logging_grid(&log);
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
        let log: RequestLog = Rc::default();
        let grid = logging_grid(&log);
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
}
