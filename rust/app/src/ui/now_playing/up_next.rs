use std::cell::Cell;

use feishin_core::queue::{QueueItem, QueueTrack};
use slint::{Model, ModelNotify, ModelTracker};

use crate::ui::QueueRow;
use crate::ui::queue::{SharedQueueView, row_at};

pub struct UpNextModel {
    queue: SharedQueueView,
    offset: Cell<Option<usize>>,
    notify: ModelNotify,
}

impl UpNextModel {
    pub(super) fn new(queue: SharedQueueView) -> Self {
        Self {
            queue,
            offset: Cell::new(None),
            notify: ModelNotify::default(),
        }
    }

    pub(super) fn external_track(&self, song_id: &str) -> Option<QueueTrack> {
        let view = self.queue.borrow();
        let is_wanted =
            |item: &&QueueItem| item.track.song_id == song_id && item.track.is_external();
        view.current()
            .filter(is_wanted)
            .or_else(|| {
                (0..view.len())
                    .filter_map(|position| view.item_at(position))
                    .find(is_wanted)
            })
            .map(|item| QueueTrack::clone(&item.track))
    }

    pub fn refresh(&self) -> Option<usize> {
        let offset = self.queue.borrow().current_position();
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
            .map_or(0, |offset| self.queue.borrow().len().saturating_sub(offset))
    }

    fn row_data(&self, row: usize) -> Option<QueueRow> {
        let position = self.offset.get()?.checked_add(row)?;
        row_at(&self.queue.borrow(), position)
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }
}

#[cfg(test)]
mod tests {
    use feishin_core::queue::{Advance, Queue};

    use super::*;
    use crate::demo::test_song;
    use crate::ui::queue::QueueModel;

    fn playing_queue() -> Queue {
        let mut queue = Queue::with_seed(3);
        let songs = (1..=3)
            .map(|index| test_song(&index.to_string(), 1_000))
            .collect();
        queue.play_now(songs, Some("2"));
        queue
    }

    #[test]
    fn up_next_starts_at_the_current_row() {
        let queue_model = QueueModel::default();
        let mut queue = playing_queue();
        queue_model.set_view(queue.view());
        let model = UpNextModel::new(queue_model.shared_view());
        assert_eq!(model.row_count(), 0);
        assert_eq!(model.refresh(), Some(1));
        assert_eq!(model.row_count(), 2);
        assert_eq!(model.row_data(0).map(|row| row.title), Some("2".into()));
        assert!(model.row_data(0).is_some_and(|row| row.current));
        queue.next(Advance::Manual);
        queue_model.set_view(queue.view());
        assert_eq!(model.refresh(), Some(2));
        assert_eq!(model.row_count(), 1);
        assert_eq!(model.row_data(0).map(|row| row.title), Some("3".into()));
        assert!(model.row_data(1).is_none());
    }
}
