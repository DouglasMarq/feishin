use feishin_api::Result;
use feishin_core::model::{Page, Paging, Song, SongListQuery, SongSort, SortOrder};

use crate::NavidromeServer;
use crate::features::Features;

const UNKNOWN_YEAR: i32 = 0;
const MAX_PROBED_YEARS: i32 = 40;
const MAX_WALKED_YEARS: usize = 10_000;
const SINGLE_SONG: Paging = Paging {
    start_index: 0,
    limit: 1,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct YearCount {
    pub(crate) year: i32,
    pub(crate) songs: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct YearSpan {
    first: i32,
    last: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DatedYears {
    undated_songs: usize,
    earliest: i32,
    latest: i32,
}

impl YearSpan {
    fn is_small(self) -> bool {
        i64::from(self.last) - i64::from(self.first) < i64::from(MAX_PROBED_YEARS)
    }

    fn requested(query: &SongListQuery) -> Option<Self> {
        match (query.year_from, query.year_to) {
            (Some(first), Some(last)) => Some(Self { first, last }),
            _ => None,
        }
    }

    fn within(query: &SongListQuery, dated: DatedYears) -> Self {
        Self {
            first: query
                .year_from
                .map_or(dated.earliest, |year| year.max(dated.earliest)),
            last: query
                .year_to
                .map_or(dated.latest, |year| year.min(dated.latest)),
        }
    }
}

pub(crate) fn in_year(query: &SongListQuery, year: i32) -> SongListQuery {
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

fn first_song_year(page: &Page<Song>) -> i32 {
    page.items
        .first()
        .and_then(|song| song.year)
        .unwrap_or(UNKNOWN_YEAR)
}

fn in_order(mut ascending: Vec<YearCount>, order: SortOrder) -> Vec<YearCount> {
    if order == SortOrder::Desc {
        ascending.reverse();
    }
    ascending
}

fn next_walked_year(year: i32, previous: i32, span: YearSpan) -> Option<i32> {
    (year > previous && year <= span.last).then_some(year)
}

impl NavidromeServer {
    pub(crate) async fn song_counts_per_year(
        &self,
        query: &SongListQuery,
        features: &Features,
    ) -> Result<Vec<YearCount>> {
        let ascending = self.ascending_year_counts(query, features).await?;
        Ok(in_order(ascending, query.order))
    }

    async fn ascending_year_counts(
        &self,
        query: &SongListQuery,
        features: &Features,
    ) -> Result<Vec<YearCount>> {
        if let Some(span) = YearSpan::requested(query).filter(|span| span.is_small()) {
            return self.probe_years(query, span, features).await;
        }
        let Some(dated) = self.dated_years(query, features).await? else {
            return Ok(Vec::new());
        };
        let span = YearSpan::within(query, dated);
        match span.is_small() {
            true => self.probe_years(query, span, features).await,
            false => self.walk_years(query, span, dated, features).await,
        }
    }

    async fn probe_years(
        &self,
        query: &SongListQuery,
        span: YearSpan,
        features: &Features,
    ) -> Result<Vec<YearCount>> {
        let mut counts = Vec::new();
        for year in span.first..=span.last {
            let songs = self.songs_in_year(query, year, features).await?;
            if songs > 0 {
                counts.push(YearCount { year, songs });
            }
        }
        Ok(counts)
    }

    async fn walk_years(
        &self,
        query: &SongListQuery,
        span: YearSpan,
        dated: DatedYears,
        features: &Features,
    ) -> Result<Vec<YearCount>> {
        let mut counts = Vec::new();
        let mut position = dated.undated_songs;
        let mut previous = UNKNOWN_YEAR;
        for _ in 0..MAX_WALKED_YEARS {
            let found = self.year_at(query, position, features).await?;
            let Some(year) = next_walked_year(found, previous, span) else {
                break;
            };
            let songs = self.songs_in_year(query, year, features).await?;
            if songs == 0 {
                break;
            }
            if year >= span.first {
                counts.push(YearCount { year, songs });
            }
            position += songs;
            previous = year;
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

    async fn year_at(
        &self,
        query: &SongListQuery,
        position: usize,
        features: &Features,
    ) -> Result<i32> {
        let oldest_first = by_year(query, SortOrder::Asc);
        let paging = Paging {
            start_index: position,
            limit: 1,
        };
        let page = self
            .native_song_page(&oldest_first, paging, features)
            .await?;
        Ok(first_song_year(&page))
    }

    async fn latest_year(&self, query: &SongListQuery, features: &Features) -> Result<i32> {
        let newest_first = by_year(query, SortOrder::Desc);
        let page = self
            .native_song_page(&newest_first, SINGLE_SONG, features)
            .await?;
        Ok(first_song_year(&page))
    }

    async fn dated_years(
        &self,
        query: &SongListQuery,
        features: &Features,
    ) -> Result<Option<DatedYears>> {
        let undated_songs = self.songs_in_year(query, UNKNOWN_YEAR, features).await?;
        let earliest = self.year_at(query, undated_songs, features).await?;
        if earliest == UNKNOWN_YEAR {
            return Ok(None);
        }
        let latest = self.latest_year(query, features).await?;
        Ok(Some(DatedYears {
            undated_songs,
            earliest,
            latest,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(year_from: Option<i32>, year_to: Option<i32>) -> SongListQuery {
        SongListQuery {
            year_from,
            year_to,
            ..SongListQuery::default()
        }
    }

    fn dated(earliest: i32, latest: i32) -> DatedYears {
        DatedYears {
            undated_songs: 0,
            earliest,
            latest,
        }
    }

    #[test]
    fn open_bounds_are_clamped_to_the_dated_songs() {
        let span = YearSpan::within(&range(None, Some(2020)), dated(1990, 2024));
        assert_eq!(
            span,
            YearSpan {
                first: 1990,
                last: 2020
            }
        );
        let span = YearSpan::within(&range(Some(1), Some(9999)), dated(1990, 2024));
        assert_eq!(
            span,
            YearSpan {
                first: 1990,
                last: 2024
            }
        );
    }

    #[test]
    fn only_short_spans_are_probed_year_by_year() {
        assert!(
            YearSpan::requested(&range(Some(1990), Some(2029))).is_some_and(YearSpan::is_small)
        );
        assert!(
            !YearSpan::requested(&range(Some(1990), Some(2030))).is_some_and(YearSpan::is_small)
        );
        assert!(!YearSpan::requested(&range(Some(1), Some(9999))).is_some_and(YearSpan::is_small));
        assert_eq!(YearSpan::requested(&range(None, Some(2020))), None);
    }

    #[test]
    fn walking_stops_past_the_span_or_when_years_stop_increasing() {
        let span = YearSpan {
            first: 1990,
            last: 2000,
        };
        assert_eq!(next_walked_year(1995, 1990, span), Some(1995));
        assert_eq!(next_walked_year(2001, 1995, span), None);
        assert_eq!(next_walked_year(1995, 1995, span), None);
        assert_eq!(next_walked_year(UNKNOWN_YEAR, 1995, span), None);
    }

    #[test]
    fn descending_order_reverses_the_counts() {
        let counts = vec![
            YearCount {
                year: 2020,
                songs: 1,
            },
            YearCount {
                year: 2021,
                songs: 2,
            },
        ];
        let years: Vec<i32> = in_order(counts, SortOrder::Desc)
            .iter()
            .map(|count| count.year)
            .collect();
        assert_eq!(years, [2021, 2020]);
    }
}
