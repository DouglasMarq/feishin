use feishin_core::model::Song;

use super::delivery::Delivery;
use super::fetching::{RetryPolicy, StreamStop};
use super::message::{StreamEvent, StreamPage};
use super::pages::{PageNumber, STREAM_PAGE_SIZE, keep_listed_rows};
use super::rows::{ListRow, RowRanges};
use super::source::PageFetcher;
use super::start::{StreamStart, start_index_in};

pub struct StreamRunner {
    pub(super) fetcher: Box<dyn PageFetcher>,
    pub(super) listed: RowRanges,
    pub(super) retry: RetryPolicy,
    delivery: Delivery,
}

struct Opened {
    page: PageNumber,
    first_page: PageNumber,
    total: Option<usize>,
    reached_end: bool,
}

fn is_last_page(received: usize, total: Option<usize>) -> bool {
    received == 0 || (total.is_none() && received < STREAM_PAGE_SIZE)
}

impl StreamRunner {
    pub fn new(
        fetcher: Box<dyn PageFetcher>,
        listed: RowRanges,
        retry: RetryPolicy,
        delivery: Delivery,
    ) -> Self {
        Self {
            fetcher,
            listed,
            retry,
            delivery,
        }
    }

    pub async fn run(mut self, start: StreamStart) {
        match self.stream(start).await {
            Ok(()) => self.finish().await,
            Err(StreamStop::Cancelled) => tracing::debug!("a newer request replaced a list stream"),
            Err(StreamStop::Failed(error)) => self.delivery.send(StreamEvent::Failed(error)),
        }
    }

    async fn finish(&mut self) {
        if self.delivery.wait_until_applied().await.is_ok() {
            self.delivery.send(StreamEvent::Finished);
        }
    }

    async fn stream(&mut self, start: StreamStart) -> Result<(), StreamStop> {
        let Some(first_row) = self.listed.first_row() else {
            return Ok(());
        };
        let opened = self.open(start, first_row).await?;
        if !opened.reached_end {
            self.stream_following(&opened).await?;
        }
        self.stream_preceding(&opened).await
    }

    async fn open(&mut self, start: StreamStart, first_row: ListRow) -> Result<Opened, StreamStop> {
        let (row, song_id) = self.start_row(start, first_row).await?;
        let (row, fetched) = self.fetch_start_page(row, first_row).await?;
        let page = PageNumber::of(row);
        let opened = Opened {
            page,
            first_page: PageNumber::of(first_row),
            total: fetched.total_count,
            reached_end: is_last_page(fetched.items.len(), fetched.total_count),
        };
        let kept = keep_listed_rows(page, fetched.items, &self.listed);
        let start_index = start_index_in(&kept, row, song_id.as_deref());
        self.delivery
            .page(kept.songs, |page| StreamEvent::Start { page, start_index })
            .await?;
        Ok(opened)
    }

    async fn stream_following(&mut self, opened: &Opened) -> Result<(), StreamStop> {
        let listed = self.listed.clone();
        let row_bound = opened
            .total
            .map_or(listed.end(), |total| total.min(listed.end()));
        let pages = opened
            .page
            .following()
            .take_while(|page| page.first_row().0 < row_bound)
            .filter(|page| listed.intersects(&page.rows()));
        for page in pages {
            let fetched = self.fetch(page).await?;
            let received = fetched.items.len();
            let kept = keep_listed_rows(page, fetched.items, &listed);
            self.deliver_unless_empty(kept.songs, StreamEvent::After)
                .await?;
            if is_last_page(received, opened.total) {
                return Ok(());
            }
        }
        Ok(())
    }

    async fn stream_preceding(&mut self, opened: &Opened) -> Result<(), StreamStop> {
        let listed = self.listed.clone();
        let pages = opened
            .page
            .preceding_down_to(opened.first_page)
            .filter(|page| listed.intersects(&page.rows()));
        for page in pages {
            let fetched = self.fetch(page).await?;
            let kept = keep_listed_rows(page, fetched.items, &listed);
            self.deliver_unless_empty(kept.songs, StreamEvent::Before)
                .await?;
        }
        Ok(())
    }

    async fn deliver_unless_empty(
        &mut self,
        songs: Vec<Song>,
        event: impl FnOnce(StreamPage) -> StreamEvent,
    ) -> Result<(), StreamStop> {
        if songs.is_empty() {
            return Ok(());
        }
        Ok(self.delivery.page(songs, event).await?)
    }
}
