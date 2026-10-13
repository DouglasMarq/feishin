use std::future::Future;

use feishin_api::Result;
use feishin_core::model::{Page, Paging};

const MAX_COUNTED_PAGES: usize = 2_000;

pub(crate) fn paginate<Item>(items: Vec<Item>, paging: Paging) -> Page<Item> {
    let total = items.len();
    let take = match paging.limit {
        0 => total,
        limit => limit,
    };
    Page {
        items: items
            .into_iter()
            .skip(paging.start_index)
            .take(take)
            .collect(),
        start_index: paging.start_index,
        total_count: Some(total),
    }
}

pub(crate) fn cloned_page<Item: Clone>(page: Page<&Item>) -> Page<Item> {
    Page {
        items: page.items.into_iter().cloned().collect(),
        start_index: page.start_index,
        total_count: page.total_count,
    }
}

pub(crate) fn subsonic_page<Item>(items: Vec<Item>, start_index: usize) -> Page<Item> {
    Page {
        items,
        start_index,
        total_count: None,
    }
}

pub(crate) async fn count_by_paging<Fetch, Pending>(page_size: usize, fetch: Fetch) -> Result<usize>
where
    Fetch: Fn(usize) -> Pending,
    Pending: Future<Output = Result<usize>>,
{
    let mut counted = 0;
    for _ in 0..MAX_COUNTED_PAGES {
        let received = fetch(counted).await?;
        counted += received;
        if received < page_size {
            return Ok(counted);
        }
    }
    tracing::debug!(counted, "stopped counting after the page limit");
    Ok(counted)
}
