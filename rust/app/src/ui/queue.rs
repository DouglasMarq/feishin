use std::cell::{Cell, RefCell};
use std::rc::Rc;

use feishin_core::queue::Uid;
use slint::{ComponentHandle, Model, ModelNotify, ModelRc, ModelTracker};

use super::{AppWindow, Context, QueueRow, QueueState, format_duration};
use crate::controller::{Command, QueueEntry};

/// Queue rows in play order; `QueueRow`s are built only for visible rows.
#[derive(Default)]
pub struct QueueModel {
    entries: RefCell<Vec<QueueEntry>>,
    current: Cell<Option<Uid>>,
    notify: ModelNotify,
}

impl QueueModel {
    fn position(&self, uid: Option<Uid>) -> Option<usize> {
        let uid = uid?;
        self.entries.borrow().iter().position(|e| e.uid == uid)
    }

    fn uid_at(&self, index: usize) -> Option<Uid> {
        self.entries.borrow().get(index).map(|e| e.uid)
    }

    /// The `move_items` arguments that drag row `from` onto row `to`.
    pub fn move_target(&self, from: usize, to: usize) -> Option<(Uid, Option<Uid>)> {
        let uid = self.uid_at(from)?;
        if from == to || self.uid_at(to).is_none() {
            return None;
        }
        let before = if to < from {
            self.uid_at(to)
        } else {
            self.uid_at(to + 1)
        };
        Some((uid, before))
    }

    pub fn set_entries(&self, entries: Vec<QueueEntry>, current: Option<Uid>) {
        *self.entries.borrow_mut() = entries;
        self.current.set(current);
        self.notify.reset();
    }

    pub fn set_current(&self, current: Option<Uid>) {
        let previous = self.position(self.current.replace(current));
        for row in [previous, self.position(current)].into_iter().flatten() {
            self.notify.row_changed(row);
        }
    }
}

impl Model for QueueModel {
    type Data = QueueRow;

    fn row_count(&self) -> usize {
        self.entries.borrow().len()
    }

    fn row_data(&self, row: usize) -> Option<QueueRow> {
        let entries = self.entries.borrow();
        let entry = entries.get(row)?;
        Some(QueueRow {
            uid: entry.uid.to_string().into(),
            title: entry.title.as_str().into(),
            artists: entry.artists.as_str().into(),
            duration: format_duration(entry.duration_ms).into(),
            current: self.current.get() == Some(entry.uid),
        })
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn with_model(window: &AppWindow, f: impl FnOnce(&QueueModel)) {
    let rows = window.global::<QueueState>().get_rows();
    if let Some(model) = rows.as_any().downcast_ref::<QueueModel>() {
        f(model);
    }
}

pub fn install(window: &AppWindow, ctx: &Context) {
    let state = window.global::<QueueState>();
    state.set_rows(ModelRc::from(Rc::new(QueueModel::default())));

    let play_ctx = ctx.clone();
    state.on_play(move |uid| {
        if let Ok(uid) = uid.parse() {
            play_ctx.send(Command::PlayUid(uid));
        }
    });
    let remove_ctx = ctx.clone();
    state.on_remove(move |uid| {
        if let Ok(uid) = uid.parse() {
            remove_ctx.send(Command::Remove(vec![uid]));
        }
    });
    let move_ctx = ctx.clone();
    let weak = window.as_weak();
    state.on_move(move |from, to| {
        let (Some(window), Ok(from), Ok(to)) =
            (weak.upgrade(), usize::try_from(from), usize::try_from(to))
        else {
            return;
        };
        with_model(&window, |model| {
            if let Some((uid, before)) = model.move_target(from, to) {
                move_ctx.send(Command::Move {
                    uids: vec![uid],
                    before,
                });
            }
        });
    });
}

pub fn set_entries(window: &AppWindow, entries: Vec<QueueEntry>, current: Option<Uid>) {
    with_model(window, |model| model.set_entries(entries, current));
}

pub fn set_current(window: &AppWindow, current: Option<Uid>) {
    with_model(window, |model| model.set_current(current));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(uid: Uid) -> QueueEntry {
        QueueEntry {
            uid,
            song_id: uid.to_string(),
            title: String::new(),
            artists: String::new(),
            duration_ms: 0,
        }
    }

    #[test]
    fn drag_targets_map_to_move_items_arguments() {
        let model = QueueModel::default();
        model.set_entries((10..14).map(entry).collect(), None);
        assert_eq!(model.move_target(3, 1), Some((13, Some(11))));
        assert_eq!(model.move_target(0, 2), Some((10, Some(13))));
        assert_eq!(model.move_target(0, 3), Some((10, None)));
        assert_eq!(model.move_target(2, 2), None);
    }
}
