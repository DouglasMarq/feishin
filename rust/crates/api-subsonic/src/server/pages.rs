use feishin_core::model::{Page, Paging};

use crate::sort::paginate;

pub fn server_page<T>(items: Vec<T>, paging: Paging, total_count: Option<usize>) -> Page<T> {
    Page {
        items,
        start_index: paging.start_index,
        total_count,
    }
}

pub fn client_page<T>(items: Vec<T>, paging: Paging) -> Page<T> {
    let total_count = items.len();
    Page {
        items: paginate(items, paging.start_index, paging.limit),
        start_index: paging.start_index,
        total_count: Some(total_count),
    }
}

pub fn cloned_page<T: Clone>(page: Page<&T>) -> Page<T> {
    Page {
        items: page.items.into_iter().cloned().collect(),
        start_index: page.start_index,
        total_count: page.total_count,
    }
}

pub fn wants_favorites_only(favorite: Option<bool>) -> bool {
    favorite == Some(true)
}

pub fn non_empty_search(search: Option<&String>) -> Option<&str> {
    search.map(String::as_str).filter(|term| !term.is_empty())
}
