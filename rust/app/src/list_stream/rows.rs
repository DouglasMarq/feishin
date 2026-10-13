use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ListRow(pub usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowRanges(Vec<Range<usize>>);

impl RowRanges {
    pub const WHOLE_LIST: Range<usize> = 0..usize::MAX;

    pub fn whole_list() -> Self {
        Self(vec![Self::WHOLE_LIST])
    }

    pub fn from_ranges(mut ranges: Vec<Range<usize>>) -> Self {
        ranges.retain(|range| !range.is_empty());
        ranges.sort_by_key(|range| range.start);
        Self(ranges.into_iter().fold(Vec::new(), merge_into))
    }

    pub fn covering(rows: &[usize]) -> Vec<Range<usize>> {
        let single_rows = rows.iter().map(|&row| row..row.saturating_add(1)).collect();
        Self::from_ranges(single_rows).0
    }

    pub fn first_row(&self) -> Option<ListRow> {
        self.0.first().map(|range| ListRow(range.start))
    }

    pub fn end(&self) -> usize {
        self.0.last().map_or(0, |range| range.end)
    }

    pub fn contains(&self, row: ListRow) -> bool {
        let candidate = self.0.partition_point(|range| range.end <= row.0);
        self.0
            .get(candidate)
            .is_some_and(|range| range.contains(&row.0))
    }

    pub fn intersects(&self, rows: &Range<usize>) -> bool {
        let candidate = self.0.partition_point(|range| range.end <= rows.start);
        self.0
            .get(candidate)
            .is_some_and(|range| range.start < rows.end)
    }

    pub fn clamped(&self, row_count: usize) -> Self {
        let clamped = self
            .0
            .iter()
            .map(|range| range.start.min(row_count)..range.end.min(row_count))
            .collect();
        Self::from_ranges(clamped)
    }

    pub fn row_count(&self) -> usize {
        self.0
            .iter()
            .fold(0, |count: usize, range| count.saturating_add(range.len()))
    }

    pub fn nth_row(&self, mut offset: usize) -> Option<ListRow> {
        for range in &self.0 {
            match range.start.checked_add(offset) {
                Some(row) if row < range.end => return Some(ListRow(row)),
                _ => offset = offset.saturating_sub(range.len()),
            }
        }
        None
    }
}

fn merge_into(mut merged: Vec<Range<usize>>, range: Range<usize>) -> Vec<Range<usize>> {
    match merged.last_mut() {
        Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
        _ => merged.push(range),
    }
    merged
}
