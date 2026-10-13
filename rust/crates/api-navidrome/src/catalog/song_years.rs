use feishin_api::Result;
use feishin_core::model::{Page, Paging, Song, SongListQuery, SongSort, SortOrder};

use crate::NavidromeServer;
use crate::features::Features;

const UNKNOWN_YEAR: i32 = 0;
const SINGLE_SONG: Paging = Paging {
    start_index: 0,
    limit: 1,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct YearCount {
    year: i32,
    songs: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct YearWindow {
    year: i32,
    paging: Paging,
}

pub(crate) fn spans_several_years(query: &SongListQuery) -> bool {
    match (query.year_from, query.year_to) {
        (None, None) => false,
        (Some(from), Some(to)) => from != to,
        _ => true,
    }
}

fn in_year(query: &SongListQuery, year: i32) -> SongListQuery {
    SongListQuery {
        year_from: Some(year),
        year_to: Some(year),
        ..query.clone()
    }
}

fn by_year(query: &SongListQuery, order: SortOrder) -> SongListQuery {
    SongListQuery {
        sort: SongSort::Year,
        order,
        year_from: None,
        year_to: None,
        ..query.clone()
    }
}

fn ordered_years(first: i32, last: i32, order: SortOrder) -> Vec<i32> {
    let years = first..=last;
    match order {
        SortOrder::Asc => years.collect(),
        SortOrder::Desc => years.rev().collect(),
    }
}

fn first_song_year(page: &Page<Song>) -> i32 {
    page.items
        .first()
        .and_then(|song| song.year)
        .unwrap_or(UNKNOWN_YEAR)
}

fn total_songs(counts: &[YearCount]) -> usize {
    counts.iter().map(|count| count.songs).sum()
}

fn year_windows(counts: &[YearCount], paging: Paging) -> Vec<YearWindow> {
    let wanted_end = match paging.limit {
        0 => usize::MAX,
        limit => paging.start_index.saturating_add(limit),
    };
    let mut windows = Vec::new();
    let mut year_start = 0;
    for count in counts {
        let year_end = year_start + count.songs;
        let start = paging.start_index.max(year_start);
        let end = wanted_end.min(year_end);
        if start < end {
            windows.push(YearWindow {
                year: count.year,
                paging: Paging {
                    start_index: start - year_start,
                    limit: end - start,
                },
            });
        }
        year_start = year_end;
    }
    windows
}

impl NavidromeServer {
    pub(crate) async fn song_page_across_years(
        &self,
        query: &SongListQuery,
        paging: Paging,
        features: &Features,
    ) -> Result<Page<Song>> {
        let counts = self.song_counts_per_year(query, features).await?;
        let mut items = Vec::new();
        for window in year_windows(&counts, paging) {
            let year_query = in_year(query, window.year);
            let page = self
                .native_song_page(&year_query, window.paging, features)
                .await?;
            items.extend(page.items);
        }
        Ok(Page {
            items,
            start_index: paging.start_index,
            total_count: Some(total_songs(&counts)),
        })
    }

    pub(crate) async fn song_count_across_years(
        &self,
        query: &SongListQuery,
        features: &Features,
    ) -> Result<usize> {
        let counts = self.song_counts_per_year(query, features).await?;
        Ok(total_songs(&counts))
    }

    async fn song_counts_per_year(
        &self,
        query: &SongListQuery,
        features: &Features,
    ) -> Result<Vec<YearCount>> {
        let first = match query.year_from {
            Some(year) => year,
            None => self.earliest_song_year(query, features).await?,
        };
        let last = match query.year_to {
            Some(year) => year,
            None => self.latest_song_year(query, features).await?,
        };
        let mut counts = Vec::new();
        for year in ordered_years(first, last, query.order) {
            let songs = self.songs_in_year(query, year, features).await?;
            if songs > 0 {
                counts.push(YearCount { year, songs });
            }
        }
        Ok(counts)
    }

    async fn songs_in_year(
        &self,
        query: &SongListQuery,
        year: i32,
        features: &Features,
    ) -> Result<usize> {
        let page = self
            .native_song_page(&in_year(query, year), SINGLE_SONG, features)
            .await?;
        Ok(page.total_count.unwrap_or(0))
    }

    async fn latest_song_year(&self, query: &SongListQuery, features: &Features) -> Result<i32> {
        let newest_first = by_year(query, SortOrder::Desc);
        let page = self
            .native_song_page(&newest_first, SINGLE_SONG, features)
            .await?;
        Ok(first_song_year(&page))
    }

    async fn earliest_song_year(&self, query: &SongListQuery, features: &Features) -> Result<i32> {
        let undated = self.songs_in_year(query, UNKNOWN_YEAR, features).await?;
        let oldest_first = by_year(query, SortOrder::Asc);
        let first_dated = Paging {
            start_index: undated,
            limit: 1,
        };
        let page = self
            .native_song_page(&oldest_first, first_dated, features)
            .await?;
        Ok(first_song_year(&page))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts() -> Vec<YearCount> {
        vec![
            YearCount {
                year: 2021,
                songs: 2,
            },
            YearCount {
                year: 2022,
                songs: 3,
            },
        ]
    }

    fn window(year: i32, start_index: usize, limit: usize) -> YearWindow {
        YearWindow {
            year,
            paging: Paging { start_index, limit },
        }
    }

    #[test]
    fn windows_split_a_page_at_year_boundaries() {
        let page = Paging {
            start_index: 1,
            limit: 3,
        };
        assert_eq!(
            year_windows(&counts(), page),
            [window(2021, 1, 1), window(2022, 0, 2)]
        );
    }

    #[test]
    fn unlimited_pages_take_every_remaining_song() {
        let page = Paging {
            start_index: 3,
            limit: 0,
        };
        assert_eq!(year_windows(&counts(), page), [window(2022, 1, 2)]);
    }

    #[test]
    fn pages_past_the_end_are_empty() {
        let page = Paging {
            start_index: 9,
            limit: 5,
        };
        assert!(year_windows(&counts(), page).is_empty());
    }

    #[test]
    fn a_range_needs_two_different_years_or_one_open_bound() {
        let range = |year_from, year_to| SongListQuery {
            year_from,
            year_to,
            ..SongListQuery::default()
        };
        assert!(spans_several_years(&range(Some(2021), Some(2022))));
        assert!(spans_several_years(&range(Some(2021), None)));
        assert!(spans_several_years(&range(None, Some(2021))));
        assert!(!spans_several_years(&range(Some(2021), Some(2021))));
        assert!(!spans_several_years(&range(None, None)));
    }

    #[test]
    fn descending_order_walks_years_backwards() {
        assert_eq!(
            ordered_years(2020, 2022, SortOrder::Desc),
            [2022, 2021, 2020]
        );
        assert!(ordered_years(2022, 2020, SortOrder::Asc).is_empty());
    }
}
