use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ScrollDirection {
    #[default]
    Forward,
    Backward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageShape {
    pub page_size: usize,
    pub capacity: usize,
}

#[derive(Debug)]
pub struct PageCache<Row> {
    shape: PageShape,
    pages: HashMap<usize, Vec<Row>>,
    requested: HashSet<usize>,
    anchor: usize,
    direction: ScrollDirection,
}

impl<Row> PageCache<Row> {
    pub fn new(shape: PageShape) -> Self {
        Self {
            shape: PageShape {
                page_size: shape.page_size.max(1),
                capacity: shape.capacity.max(1),
            },
            pages: HashMap::new(),
            requested: HashSet::new(),
            anchor: 0,
            direction: ScrollDirection::default(),
        }
    }

    pub fn page_of(&self, index: usize) -> usize {
        index / self.shape.page_size
    }

    pub fn first_index_of(&self, page: usize) -> usize {
        page.saturating_mul(self.shape.page_size)
    }

    pub fn get(&self, index: usize) -> Option<&Row> {
        self.pages
            .get(&self.page_of(index))?
            .get(index % self.shape.page_size)
    }

    pub fn rows(&self) -> impl Iterator<Item = (usize, &Row)> {
        let page_size = self.shape.page_size;
        self.pages.iter().flat_map(move |(page, rows)| {
            rows.iter()
                .enumerate()
                .map(move |(position, row)| (global_index(*page, page_size, position), row))
        })
    }

    pub fn rows_mut(&mut self) -> impl Iterator<Item = (usize, &mut Row)> {
        let page_size = self.shape.page_size;
        self.pages.iter_mut().flat_map(move |(page, rows)| {
            rows.iter_mut()
                .enumerate()
                .map(move |(position, row)| (global_index(*page, page_size, position), row))
        })
    }

    pub fn mark_requested(&mut self, page: usize) -> bool {
        self.requested.insert(page)
    }

    pub fn forget_request(&mut self, page: usize) -> bool {
        self.requested.remove(&page)
    }

    pub fn pages_to_fetch(&mut self, index: usize, total: usize) -> Vec<usize> {
        let page = self.page_of(index);
        self.move_anchor(page);
        let neighbour = match self.direction {
            ScrollDirection::Forward => page.checked_add(1),
            ScrollDirection::Backward => page.checked_sub(1),
        };
        let page_size = self.shape.page_size;
        [Some(page), neighbour]
            .into_iter()
            .flatten()
            .filter(|candidate| {
                candidate
                    .checked_mul(page_size)
                    .is_some_and(|start| start < total)
            })
            .filter(|candidate| self.requested.insert(*candidate))
            .collect()
    }

    fn move_anchor(&mut self, page: usize) {
        self.direction = match page.cmp(&self.anchor) {
            std::cmp::Ordering::Greater => ScrollDirection::Forward,
            std::cmp::Ordering::Less => ScrollDirection::Backward,
            std::cmp::Ordering::Equal => self.direction,
        };
        self.anchor = page;
    }

    pub fn insert(&mut self, page: usize, rows: Vec<Row>) {
        self.requested.insert(page);
        self.pages.insert(page, rows);
        while self.pages.len() > self.shape.capacity {
            let Some(farthest) = self.farthest_page() else {
                return;
            };
            self.pages.remove(&farthest);
            self.requested.remove(&farthest);
        }
    }

    fn farthest_page(&self) -> Option<usize> {
        let anchor = self.anchor;
        self.pages
            .keys()
            .copied()
            .max_by_key(|page| (page.abs_diff(anchor), *page))
    }
}

fn global_index(page: usize, page_size: usize, position: usize) -> usize {
    page.saturating_mul(page_size).saturating_add(position)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHAPE: PageShape = PageShape {
        page_size: 10,
        capacity: 3,
    };

    fn rows_for(cache: &PageCache<usize>, page: usize) -> Vec<usize> {
        let start = cache.first_index_of(page);
        (start..start + cache.shape.page_size).collect()
    }

    fn load(cache: &mut PageCache<usize>, index: usize, total: usize) -> Vec<usize> {
        let pages = cache.pages_to_fetch(index, total);
        for page in &pages {
            let rows = rows_for(cache, *page);
            cache.insert(*page, rows);
        }
        pages
    }

    fn loaded(cache: &PageCache<usize>) -> Vec<usize> {
        let mut pages: Vec<usize> = cache.pages.keys().copied().collect();
        pages.sort_unstable();
        pages
    }

    #[test]
    fn requests_the_page_and_prefetches_the_next_one_once() {
        let mut cache = PageCache::<usize>::new(SHAPE);
        assert_eq!(cache.pages_to_fetch(5, 100), [0, 1]);
        assert!(cache.pages_to_fetch(7, 100).is_empty());
        assert_eq!(cache.pages_to_fetch(15, 100), [2]);
        assert_eq!(cache.pages_to_fetch(95, 100), [9]);
        assert_eq!(cache.pages_to_fetch(55, 100), [5, 4]);
        assert_eq!(cache.get(5), None);
    }

    #[test]
    fn evicts_the_page_farthest_from_the_last_request() {
        let mut cache = PageCache::<usize>::new(SHAPE);
        load(&mut cache, 0, 1000);
        load(&mut cache, 25, 1000);
        assert_eq!(loaded(&cache), [1, 2, 3]);
        assert_eq!(cache.get(25), Some(&25));
        assert_eq!(cache.get(5), None);
        assert!(!cache.requested.contains(&0));

        load(&mut cache, 85, 1000);
        assert_eq!(loaded(&cache), [3, 8, 9]);
        load(&mut cache, 45, 1000);
        assert_eq!(loaded(&cache), [3, 4, 8]);
        assert_eq!(cache.pages.len(), SHAPE.capacity);
    }

    #[test]
    fn evicted_pages_are_fetched_again_when_scrolled_back() {
        let mut cache = PageCache::<usize>::new(SHAPE);
        load(&mut cache, 0, 1000);
        load(&mut cache, 50, 1000);
        load(&mut cache, 70, 1000);
        assert_eq!(loaded(&cache), [6, 7, 8]);
        assert_eq!(cache.get(5), None);
        assert_eq!(load(&mut cache, 5, 1000), [0]);
        assert_eq!(cache.get(5), Some(&5));
        assert_eq!(loaded(&cache), [0, 6, 7]);
    }

    #[test]
    fn late_pages_far_from_the_anchor_are_dropped_and_requestable() {
        let mut cache = PageCache::<usize>::new(SHAPE);
        let early = cache.pages_to_fetch(0, 1000);
        load(&mut cache, 50, 1000);
        load(&mut cache, 60, 1000);
        for page in early {
            let rows = rows_for(&cache, page);
            cache.insert(page, rows);
        }
        assert_eq!(loaded(&cache), [5, 6, 7]);
        assert!(!cache.requested.contains(&0));
        assert!(cache.forget_request(7));
        assert!(cache.mark_requested(7));
    }

    #[test]
    fn rows_report_their_global_index() {
        let mut cache = PageCache::<usize>::new(SHAPE);
        load(&mut cache, 30, 1000);
        let mut indexes: Vec<usize> = cache.rows().map(|(index, _)| index).collect();
        indexes.sort_unstable();
        assert_eq!(indexes, (30..50).collect::<Vec<_>>());
        for (index, row) in cache.rows_mut() {
            *row = index * 2;
        }
        assert_eq!(cache.get(41), Some(&82));
    }
}
