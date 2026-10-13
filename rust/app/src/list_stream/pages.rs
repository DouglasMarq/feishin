use std::ops::Range;

use feishin_core::model::{Paging, Song};

use super::rows::{ListRow, RowRanges};

pub const STREAM_PAGE_SIZE: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PageNumber(usize);

impl PageNumber {
    pub fn of(row: ListRow) -> Self {
        Self(row.0 / STREAM_PAGE_SIZE)
    }

    pub fn first_row(self) -> ListRow {
        ListRow(self.0.saturating_mul(STREAM_PAGE_SIZE))
    }

    pub fn rows(self) -> Range<usize> {
        let start = self.first_row().0;
        start..start.saturating_add(STREAM_PAGE_SIZE)
    }

    pub fn paging(self) -> Paging {
        Paging {
            start_index: self.first_row().0,
            limit: STREAM_PAGE_SIZE,
        }
    }

    pub fn following(self) -> impl Iterator<Item = Self> {
        (self.0.saturating_add(1)..usize::MAX).map(Self)
    }

    pub fn preceding_down_to(self, first: Self) -> impl Iterator<Item = Self> {
        (first.0..self.0).rev().map(Self)
    }
}

pub struct KeptSongs {
    pub rows: Vec<ListRow>,
    pub songs: Vec<Song>,
}

pub fn keep_listed_rows(page: PageNumber, songs: Vec<Song>, listed: &RowRanges) -> KeptSongs {
    let first_row = page.first_row().0;
    let (rows, songs) = songs
        .into_iter()
        .enumerate()
        .map(|(offset, song)| (ListRow(first_row.saturating_add(offset)), song))
        .filter(|(row, _)| listed.contains(*row))
        .unzip();
    KeptSongs { rows, songs }
}
