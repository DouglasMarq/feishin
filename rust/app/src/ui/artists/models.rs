use std::rc::Rc;

use slint::{Model, ModelRc, ModelTracker, VecModel};

use super::super::{ArtistCard, ArtistGridRow};
use super::list::ArtistList;

pub struct ListModel {
    pub list: Rc<ArtistList>,
}

impl Model for ListModel {
    type Data = ArtistCard;

    fn row_count(&self) -> usize {
        self.list.len()
    }

    fn row_data(&self, row: usize) -> Option<ArtistCard> {
        self.list.card(row)
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        self.list.list_notify()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub struct GridModel {
    pub list: Rc<ArtistList>,
}

impl Model for GridModel {
    type Data = ArtistGridRow;

    fn row_count(&self) -> usize {
        self.list.grid_rows(self.list.len())
    }

    fn row_data(&self, row: usize) -> Option<ArtistGridRow> {
        let columns = self.list.columns();
        let start = row * columns;
        let end = (start + columns).min(self.list.len());
        if start >= end {
            return None;
        }
        let cards: Vec<ArtistCard> = (start..end)
            .filter_map(|index| self.list.card(index))
            .collect();
        Some(ArtistGridRow {
            cards: ModelRc::new(VecModel::from(cards)),
        })
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        self.list.grid_notify()
    }
}

#[cfg(test)]
mod tests {
    use feishin_core::model::ArtistListQuery;

    use super::super::list::tests::{items, list};
    use super::*;

    #[test]
    fn grid_groups_rows_by_columns() {
        let (list, _log) = list();
        list.reset(ArtistListQuery::default());
        list.set_columns(4);
        assert!(list.apply_page(1, 0, items(0, 10)));
        let grid = GridModel {
            list: Rc::new(list),
        };
        assert_eq!(grid.row_count(), 3);
        assert_eq!(grid.row_data(2).map(|row| row.cards.row_count()), Some(2));
        assert!(grid.row_data(3).is_none());
        grid.list.set_columns(5);
        assert_eq!(grid.row_count(), 2);
    }
}
