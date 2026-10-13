use feishin_core::model::Paging;

pub const MORE_PAGE: usize = 50;

#[derive(Debug, PartialEq, Eq)]
pub enum Applied {
    Replaced,
    Appended(usize),
}

impl Applied {
    pub fn first_new_index(&self) -> usize {
        match self {
            Self::Replaced => 0,
            Self::Appended(start) => *start,
        }
    }
}

#[derive(Debug)]
pub struct Section<Item> {
    pub items: Vec<Item>,
    offset: Option<usize>,
    pub has_more: bool,
    pub busy: bool,
}

impl<Item> Default for Section<Item> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            offset: None,
            has_more: false,
            busy: false,
        }
    }
}

impl<Item> Section<Item> {
    pub fn new(items: Vec<Item>, limit: usize) -> Self {
        Self {
            has_more: items.len() >= limit,
            items,
            ..Self::default()
        }
    }

    pub fn next_page(&mut self) -> Option<Paging> {
        if self.busy || !self.has_more {
            return None;
        }
        self.busy = true;
        Some(match self.offset {
            None => Paging {
                start_index: 0,
                limit: self.items.len() + MORE_PAGE,
            },
            Some(start_index) => Paging {
                start_index,
                limit: MORE_PAGE,
            },
        })
    }

    pub fn apply(&mut self, paging: Paging, items: Vec<Item>) -> Applied {
        self.busy = false;
        self.has_more = items.len() >= paging.limit;
        self.offset = Some(paging.start_index + items.len());
        match paging.start_index {
            0 => {
                self.items = items;
                Applied::Replaced
            }
            _ => {
                let start = self.items.len();
                self.items.extend(items);
                Applied::Appended(start)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::search::query::SEARCH_LIMIT;
    use crate::ui::search::test_support::numbered_songs;

    #[test]
    fn show_more_refetches_once_then_pages() {
        let mut section = Section::new(numbered_songs(SEARCH_LIMIT), SEARCH_LIMIT);
        assert!(section.has_more);
        let first = section.next_page();
        assert_eq!(
            first,
            Some(Paging {
                start_index: 0,
                limit: SEARCH_LIMIT + MORE_PAGE
            })
        );
        assert_eq!(section.next_page(), None, "busy sections do not refetch");

        let paging = first.unwrap_or(Paging {
            start_index: 0,
            limit: 0,
        });
        assert_eq!(
            section.apply(paging, numbered_songs(paging.limit)),
            Applied::Replaced
        );
        assert_eq!(section.items.len(), SEARCH_LIMIT + MORE_PAGE);

        let second = section.next_page();
        assert_eq!(
            second,
            Some(Paging {
                start_index: SEARCH_LIMIT + MORE_PAGE,
                limit: MORE_PAGE
            })
        );
        let paging = second.unwrap_or(paging);
        assert_eq!(
            section.apply(paging, numbered_songs(3)),
            Applied::Appended(SEARCH_LIMIT + MORE_PAGE)
        );
        assert!(!section.has_more);
        assert_eq!(section.next_page(), None);
    }

    #[test]
    fn short_results_have_no_more() {
        let section = Section::new(numbered_songs(3), SEARCH_LIMIT);
        assert!(!section.has_more);
    }
}
