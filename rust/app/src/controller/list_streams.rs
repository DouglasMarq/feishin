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
        for (_, active) in self.active.drain() {
            active.task.abort();
        }
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.active.is_empty()
    }
}
