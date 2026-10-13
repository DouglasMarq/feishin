use std::collections::HashMap;
use std::ops::Range;
use std::time::{Duration, Instant};

use feishin_core::queue::{InsertAnchor, Queue, Uid};
use tokio::task::AbortHandle;

use crate::list_stream::{Placement, StreamId};

const QUEUE_PUBLISH_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PageSide {
    After,
    Before,
}

pub(super) struct ActiveStream {
    placement: Placement,
    owned: Vec<Range<Uid>>,
    head: Option<Range<Uid>>,
    tail: Option<Range<Uid>>,
    task: AbortHandle,
    published_at: Option<Instant>,
}

impl ActiveStream {
    pub(super) fn new(placement: Placement, task: AbortHandle) -> Self {
        Self {
            placement,
            owned: Vec::new(),
            head: None,
            tail: None,
            task,
            published_at: None,
        }
    }

    pub(super) fn placement(&self) -> Placement {
        self.placement
    }

    pub(super) fn is_started(&self) -> bool {
        !self.owned.is_empty()
    }

    pub(super) fn record_start(&mut self, inserted: Range<Uid>) {
        self.head = Some(inserted.clone());
        self.record(PageSide::After, inserted);
        self.published_at = Some(Instant::now());
    }

    pub(super) fn record(&mut self, side: PageSide, inserted: Range<Uid>) {
        match side {
            PageSide::After => self.tail = Some(inserted.clone()),
            PageSide::Before => self.head = Some(inserted.clone()),
        }
        self.owned.push(inserted);
    }

    pub(super) fn owns(&self, uid: Uid) -> bool {
        let candidate = self.owned.partition_point(|range| range.end <= uid);
        self.owned
            .get(candidate)
            .is_some_and(|range| range.contains(&uid))
    }

    pub(super) fn anchor(&self, side: PageSide, queue: &Queue) -> InsertAnchor {
        match side {
            PageSide::After => self.after_anchor(queue),
            PageSide::Before => self.before_anchor(queue),
        }
    }

    fn after_anchor(&self, queue: &Queue) -> InsertAnchor {
        self.tail
            .clone()
            .and_then(|tail| tail.rev().find(|uid| queue.contains(*uid)))
            .map_or_else(|| self.first_anchor(queue), InsertAnchor::After)
    }

    fn before_anchor(&self, queue: &Queue) -> InsertAnchor {
        self.head
            .clone()
            .and_then(|mut head| head.find(|uid| queue.contains(*uid)))
            .map_or(InsertAnchor::Start, InsertAnchor::Before)
    }

    fn first_anchor(&self, queue: &Queue) -> InsertAnchor {
        let current = queue.current().map(|item| item.uid);
        match (self.placement, current) {
            (Placement::Next, Some(current)) => InsertAnchor::After(current),
            (Placement::Next, None) => InsertAnchor::Start,
            (Placement::Now | Placement::Last, _) => InsertAnchor::End,
        }
    }

    pub(super) fn publish_due(&mut self) -> bool {
        match self.published_at {
            Some(published_at) if published_at.elapsed() < QUEUE_PUBLISH_INTERVAL => false,
            _ => {
                self.published_at = Some(Instant::now());
                true
            }
        }
    }
}

impl Drop for ActiveStream {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Default)]
pub(super) struct ListStreams {
    last_id: u64,
    active: HashMap<StreamId, ActiveStream>,
}

impl ListStreams {
    pub(super) fn next_id(&mut self) -> StreamId {
        self.last_id += 1;
        StreamId(self.last_id)
    }

    pub(super) fn insert(&mut self, stream: StreamId, active: ActiveStream) {
        self.active.insert(stream, active);
    }

    pub(super) fn get_mut(&mut self, stream: StreamId) -> Option<&mut ActiveStream> {
        self.active.get_mut(&stream)
    }

    pub(super) fn remove(&mut self, stream: StreamId) -> Option<ActiveStream> {
        self.active.remove(&stream)
    }

    pub(super) fn cancel_all(&mut self) {
        self.active.clear();
    }

    pub(super) fn cancel_unstarted_plays(&mut self) {
        self.active
            .retain(|_, active| active.placement != Placement::Now || active.is_started());
    }

    pub(super) fn cancel_older_than(&mut self, newest: StreamId) {
        self.active.retain(|stream, _| stream.0 >= newest.0);
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.active.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idle_task() -> AbortHandle {
        tokio::spawn(std::future::pending::<()>()).abort_handle()
    }

    #[tokio::test]
    async fn dropping_a_stream_stops_its_task() {
        let task = tokio::spawn(std::future::pending::<()>());
        drop(ActiveStream::new(Placement::Now, task.abort_handle()));
        assert!(task.await.is_err_and(|error| error.is_cancelled()));
    }

    #[tokio::test]
    async fn a_new_play_only_cancels_plays_that_have_not_started() {
        let mut streams = ListStreams::default();
        let started = streams.next_id();
        let mut started_stream = ActiveStream::new(Placement::Now, idle_task());
        started_stream.record_start(1..3);
        streams.insert(started, started_stream);
        let pending = streams.next_id();
        streams.insert(pending, ActiveStream::new(Placement::Now, idle_task()));
        let enqueued = streams.next_id();
        streams.insert(enqueued, ActiveStream::new(Placement::Last, idle_task()));

        streams.cancel_unstarted_plays();
        assert!(streams.get_mut(started).is_some());
        assert!(streams.get_mut(pending).is_none());
        assert!(streams.get_mut(enqueued).is_some());

        streams.cancel_older_than(enqueued);
        assert!(streams.get_mut(started).is_none());
        assert!(streams.get_mut(enqueued).is_some());
    }
}
