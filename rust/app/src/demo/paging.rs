use feishin_core::model::{Page, Paging};

pub(super) fn paginate<Item>(
    items: impl ExactSizeIterator<Item = Item>,
    paging: Paging,
) -> Page<Item> {
    let total = items.len();
    Page {
        items: items.skip(paging.start_index).take(paging.limit).collect(),
        start_index: paging.start_index,
        total_count: Some(total),
    }
}

pub(super) fn paginate_order<Item>(
    order: &[usize],
    paging: Paging,
    to_item: impl Fn(usize) -> Item,
) -> Page<Item> {
    Page {
        items: page_slice(order, paging)
            .iter()
            .map(|&index| to_item(index))
            .collect(),
        start_index: paging.start_index,
        total_count: Some(order.len()),
    }
}

fn page_slice(order: &[usize], paging: Paging) -> &[usize] {
    let start = paging.start_index.min(order.len());
    let end = start.saturating_add(paging.limit).min(order.len());
    order.get(start..end).unwrap_or_default()
}
