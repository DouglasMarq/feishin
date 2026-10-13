use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use feishin_api::ApiError;
use feishin_core::model::{Page, Paging, Song};
use tokio::sync::mpsc;

use crate::demo::test_song;
use crate::list_stream::delivery::{Deliver, Delivery};
use crate::list_stream::fetching::RetryPolicy;
use crate::list_stream::message::{StreamEvent, StreamId, StreamMessage};
use crate::list_stream::rows::RowRanges;
use crate::list_stream::runner::StreamRunner;
use crate::list_stream::source::PageFetcher;
use crate::list_stream::start::StreamStart;

pub const IMPATIENT: RetryPolicy = RetryPolicy {
    attempts: 3,
    first_delay: Duration::ZERO,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recorded {
    Start(Vec<String>, usize),
    After(Vec<String>),
    Before(Vec<String>),
    Failed,
    Finished,
}

#[derive(Default)]
pub struct Failures {
    pub failing_row: Option<usize>,
    pub remaining: Option<u32>,
}

#[derive(Clone)]
pub struct FakePages {
    pub song_count: usize,
    pub reports_total: bool,
    pub failures: Arc<Mutex<Failures>>,
    pub requested_rows: Arc<Mutex<Vec<usize>>>,
}

impl FakePages {
    pub fn new(song_count: usize) -> Self {
        Self {
            song_count,
            reports_total: true,
            failures: Arc::default(),
            requested_rows: Arc::default(),
        }
    }

    pub fn failing_at(self, row: usize, times: Option<u32>) -> Self {
        if let Ok(mut failures) = self.failures.lock() {
            failures.failing_row = Some(row);
            failures.remaining = times;
        }
        self
    }

    pub fn requested(&self) -> Vec<usize> {
        self.requested_rows
            .lock()
            .map(|rows| rows.clone())
            .unwrap_or_default()
    }

    fn should_fail(&self, start_index: usize) -> bool {
        let Ok(mut failures) = self.failures.lock() else {
            return false;
        };
        match (failures.failing_row, failures.remaining) {
            (Some(row), None) => row == start_index,
            (Some(row), Some(remaining)) if row == start_index && remaining > 0 => {
                failures.remaining = Some(remaining - 1);
                true
            }
            _ => false,
        }
    }
}

#[async_trait]
impl PageFetcher for FakePages {
    async fn fetch(&self, paging: Paging) -> feishin_api::Result<Page<Song>> {
        if let Ok(mut rows) = self.requested_rows.lock() {
            rows.push(paging.start_index);
        }
        if self.should_fail(paging.start_index) {
            return Err(ApiError::NotFound);
        }
        let end = (paging.start_index + paging.limit).min(self.song_count);
        let items = (paging.start_index.min(end)..end)
            .map(|row| test_song(&format!("song-{row}"), 1000))
            .collect();
        Ok(Page {
            items,
            start_index: paging.start_index,
            total_count: self.reports_total.then_some(self.song_count),
        })
    }
}

pub fn ids(from: usize, to: usize) -> Vec<String> {
    (from..to).map(|row| format!("song-{row}")).collect()
}

fn page_ids(songs: &[Song]) -> Vec<String> {
    songs.iter().map(|song| song.id.clone()).collect()
}

fn record(event: StreamEvent) -> Recorded {
    match event {
        StreamEvent::Start { page, start_index } => {
            let ids = page_ids(&page.songs);
            page.applied.confirm();
            Recorded::Start(ids, start_index)
        }
        StreamEvent::After(page) => {
            let ids = page_ids(&page.songs);
            page.applied.confirm();
            Recorded::After(ids)
        }
        StreamEvent::Before(page) => {
            let ids = page_ids(&page.songs);
            page.applied.confirm();
            Recorded::Before(ids)
        }
        StreamEvent::Failed(_) => Recorded::Failed,
        StreamEvent::Finished => Recorded::Finished,
    }
}

pub fn spawn_runner(
    pages: FakePages,
    listed: RowRanges,
    start: StreamStart,
) -> mpsc::UnboundedReceiver<StreamMessage> {
    let (sender, receiver) = mpsc::unbounded_channel();
    let deliver: Deliver = Box::new(move |message| {
        let _ = sender.send(message);
    });
    let delivery = Delivery::new(StreamId(1), deliver);
    let runner = StreamRunner::new(Box::new(pages), listed, IMPATIENT, delivery);
    tokio::spawn(runner.run(start));
    receiver
}

pub async fn run_to_end(pages: FakePages, listed: RowRanges, start: StreamStart) -> Vec<Recorded> {
    let mut messages = spawn_runner(pages, listed, start);
    let mut recorded = Vec::new();
    while let Some(message) = messages.recv().await {
        recorded.push(record(message.event));
    }
    recorded
}
