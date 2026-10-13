use std::cell::Cell;

use slint::{Model, ModelNotify, ModelRc, ModelTracker};

use crate::ui::QueueRow;

pub struct UpNextModel {
    queue: ModelRc<QueueRow>,
    offset: Cell<Option<usize>>,
    notify: ModelNotify,
}

impl UpNextModel {
    pub(super) fn new(queue: ModelRc<QueueRow>) -> Self {
        Self {
            queue,
            offset: Cell::new(None),
            notify: ModelNotify::default(),
        }
    }

    fn is_current(&self, index: usize) -> bool {
        self.queue.row_data(index).is_some_and(|row| row.current)
    }

    fn locate(&self) -> Option<usize> {
        if let Some(offset) = self.offset.get()
            && self.is_current(offset)
        {
            return Some(offset);
        }
        (0..self.queue.row_count()).find(|&index| self.is_current(index))
    }

    pub fn refresh(&self) -> Option<usize> {
        let offset = self.locate();
        self.offset.set(offset);
        self.notify.reset();
        offset
    }
}

impl Model for UpNextModel {
    type Data = QueueRow;

    fn row_count(&self) -> usize {
        self.offset
            .get()
            .map_or(0, |offset| self.queue.row_count().saturating_sub(offset))
    }

    fn row_data(&self, row: usize) -> Option<QueueRow> {
        self.queue.row_data(self.offset.get()?.checked_add(row)?)
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use slint::VecModel;

    use super::*;

    fn queue_row(uid: &str, current: bool) -> QueueRow {
        QueueRow {
            uid: uid.into(),
            current,
            ..QueueRow::default()
        }
    }

    #[test]
    fn up_next_starts_at_the_current_row() {
        let queue = Rc::new(VecModel::from(vec![
            queue_row("1", false),
            queue_row("2", true),
            queue_row("3", false),
        ]));
        let model = UpNextModel::new(ModelRc::from(Rc::clone(&queue)));
        assert_eq!(model.row_count(), 0);
        model.refresh();
        assert_eq!(model.row_count(), 2);
        assert_eq!(model.row_data(0).map(|row| row.uid), Some("2".into()));
        queue.set_row_data(1, queue_row("2", false));
        queue.set_row_data(2, queue_row("3", true));
        model.refresh();
        assert_eq!(model.row_count(), 1);
        assert_eq!(model.row_data(0).map(|row| row.uid), Some("3".into()));
    }
}
