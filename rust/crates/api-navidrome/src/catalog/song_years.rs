use feishin_api::Result;
use feishin_core::model::{Page, Paging, Song, SongListQuery};

use crate::NavidromeServer;
use crate::catalog::year_counts::{YearCount, in_year};
use crate::features::Features;

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
}
