use std::future::Future;

use feishin_api::Result;

pub const MAX_ITEMS: usize = 500;
const PROBE_STRIDE: usize = MAX_ITEMS * 10;

pub async fn count_by_paging<F, Fut>(mut start: usize, fetch_page_length: F) -> Result<usize>
where
    F: Fn(usize, usize) -> Fut,
    Fut: Future<Output = Result<usize>>,
{
    loop {
        let received = fetch_page_length(start, MAX_ITEMS).await?;
        start += received;
        if received != MAX_ITEMS {
            return Ok(start);
        }
    }
}

pub async fn count_by_probing_then_paging<F, Fut>(fetch_page_length: F) -> Result<usize>
where
    F: Fn(usize, usize) -> Fut,
    Fut: Future<Output = Result<usize>>,
{
    let mut section_start = 0;
    while fetch_page_length(section_start, 1).await? == 1 {
        section_start += PROBE_STRIDE;
    }
    count_by_paging(
        section_start.saturating_sub(PROBE_STRIDE),
        fetch_page_length,
    )
    .await
}
