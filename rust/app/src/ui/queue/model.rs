use std::cell::RefCell;
use std::cmp::Ordering;
use std::rc::Rc;

use feishin_core::queue::{QueueItem, QueueView, Uid};
use slint::{Model, ModelNotify, ModelTracker};

use crate::ui::{QueueRow, format_duration};

pub type SharedQueueView = Rc<RefCell<QueueView>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewChange {
    CurrentMoved,
    LayoutReplaced,
}

fn queue_row(item: &QueueItem, current: bool) -> QueueRow {
    QueueRow {
        uid: item.uid.to_string().into(),
        title: item.track.title.as_ref().into(),
        artists: item.track.artist_names().into(),
        duration: format_duration(item.track.duration_ms).into(),
        current,
    }
}

pub fn row_at(view: &QueueView, position: usize) -> Option<QueueRow> {
    let current = view.current_position() == Some(position);
    view.item_at(position).map(|item| queue_row(item, current))
}

#[derive(Default)]
pub struct QueueModel {
    view: SharedQueueView,
    notify: ModelNotify,
}

impl QueueModel {
    pub fn shared_view(&self) -> SharedQueueView {
        Rc::clone(&self.view)
    }

    fn uid_at(&self, position: usize) -> Option<Uid> {
        self.view.borrow().item_at(position).map(|item| item.uid)
    }

    pub fn move_target(&self, from: usize, to: usize) -> Option<(Uid, Option<Uid>)> {
        let uid = self.uid_at(from)?;
        if from == to || self.uid_at(to).is_none() {
            return None;
        }
        let before = match to.cmp(&from) {
            Ordering::Less => self.uid_at(to),
            Ordering::Equal | Ordering::Greater => self.uid_at(to + 1),
        };
        Some((uid, before))
    }

    pub fn set_view(&self, view: QueueView) -> ViewChange {
        let new_position = view.current_position();
        let previous = self.view.replace(view);
        let same_layout = previous.shares_layout_with(&self.view.borrow());
        match same_layout {
            true => {
                [previous.current_position(), new_position]
                    .into_iter()
                    .flatten()
                    .for_each(|row| self.notify.row_changed(row));
                ViewChange::CurrentMoved
            }
            false => {
                self.notify.reset();
                ViewChange::LayoutReplaced
            }
        }
    }
}

impl Model for QueueModel {
    type Data = QueueRow;

    fn row_count(&self) -> usize {
        self.view.borrow().len()
    }

    fn row_data(&self, row: usize) -> Option<QueueRow> {
        row_at(&self.view.borrow(), row)
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
