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
