use std::future::Future;

use feishin_api::Result;

pub const MAX_ITEMS: usize = 500;
const COUNT_CEILING: usize = 100_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CountBounds {
    present: usize,
    absent: usize,
}

impl CountBounds {
    fn gap(self) -> usize {
        self.absent.saturating_sub(self.present)
    }

    fn middle(self) -> usize {
        self.present + self.gap() / 2
    }
}

async fn item_exists<Fetch, Pending>(fetch_page_length: &Fetch, index: usize) -> Result<bool>
where
    Fetch: Fn(usize, usize) -> Pending,
    Pending: Future<Output = Result<usize>>,
{
    Ok(fetch_page_length(index, 1).await? > 0)
}

async fn gallop<Fetch, Pending>(fetch_page_length: &Fetch) -> Result<CountBounds>
where
    Fetch: Fn(usize, usize) -> Pending,
    Pending: Future<Output = Result<usize>>,
{
    let mut bounds = CountBounds {
        present: 0,
        absent: MAX_ITEMS,
    };
    while bounds.absent < COUNT_CEILING && item_exists(fetch_page_length, bounds.absent).await? {
        bounds = CountBounds {
            present: bounds.absent + 1,
            absent: bounds.absent.saturating_mul(2),
        };
    }
    Ok(bounds)
}

async fn narrow<Fetch, Pending>(
    mut bounds: CountBounds,
    fetch_page_length: &Fetch,
) -> Result<CountBounds>
where
    Fetch: Fn(usize, usize) -> Pending,
    Pending: Future<Output = Result<usize>>,
{
    while bounds.gap() > MAX_ITEMS {
        let middle = bounds.middle();
        bounds = match item_exists(fetch_page_length, middle).await? {
            true => CountBounds {
                present: middle + 1,
                ..bounds
            },
            false => CountBounds {
                absent: middle,
                ..bounds
            },
        };
    }
    Ok(bounds)
}

pub async fn count_by_probing<Fetch, Pending>(fetch_page_length: Fetch) -> Result<usize>
where
    Fetch: Fn(usize, usize) -> Pending,
    Pending: Future<Output = Result<usize>>,
{
    let bounds = gallop(&fetch_page_length).await?;
    let bounds = narrow(bounds, &fetch_page_length).await?;
    let remaining = fetch_page_length(bounds.present, MAX_ITEMS).await?;
    Ok(bounds.present + remaining)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    struct FakeListing {
        total: usize,
        requests: AtomicUsize,
        fetched_items: AtomicUsize,
    }

    impl FakeListing {
        fn new(total: usize) -> Self {
            Self {
                total,
                requests: AtomicUsize::new(0),
                fetched_items: AtomicUsize::new(0),
            }
        }

        async fn page_length(&self, offset: usize, count: usize) -> Result<usize> {
            let length = self.total.saturating_sub(offset).min(count);
            self.requests.fetch_add(1, Ordering::SeqCst);
            self.fetched_items.fetch_add(length, Ordering::SeqCst);
            Ok(length)
        }

        async fn counted(&self) -> Option<usize> {
            count_by_probing(|offset, count| self.page_length(offset, count))
                .await
                .ok()
        }
    }

    #[tokio::test]
    async fn counts_exactly_around_page_boundaries() {
        for total in [0, 1, 499, 500, 501, 999, 1000, 1001, 12_345, 100_000] {
            assert_eq!(FakeListing::new(total).counted().await, Some(total));
        }
    }

    #[tokio::test]
    async fn large_listings_transfer_at_most_one_page_of_items() {
        let listing = FakeListing::new(100_000);
        assert_eq!(listing.counted().await, Some(100_000));
        assert!(listing.fetched_items.load(Ordering::SeqCst) <= 2 * MAX_ITEMS);
        assert!(listing.requests.load(Ordering::SeqCst) <= 20);
    }
}
