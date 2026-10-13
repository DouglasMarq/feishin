use feishin_core::model::{Page, Paging};

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
