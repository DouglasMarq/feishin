use feishin_api::{ApiError, Result};
use feishin_core::model::{Album, AlbumListQuery, Page, Paging, SortOrder};

use crate::NavidromeServer;
use crate::catalog::SUBSONIC_MAX_ITEMS;
use crate::catalog::paging::{count_by_paging, subsonic_page};
use crate::query::{QueryParams, SubsonicAlbumListType};
use crate::subsonic;

const EARLIEST_YEAR: i32 = 1;
const LATEST_YEAR: i32 = 9999;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct YearRange {
    first: i32,
    last: i32,
}

impl YearRange {
    pub(crate) fn of(query: &AlbumListQuery) -> Option<Self> {
        match (query.year_from, query.year_to) {
            (None, None) => None,
            (Some(from), Some(to)) if from == to => None,
            (from, to) => Some(Self {
                first: from.unwrap_or(EARLIEST_YEAR),
                last: to.unwrap_or(LATEST_YEAR),
            }),
        }
    }

    fn bounds_in(self, order: SortOrder) -> (i32, i32) {
        match order {
            SortOrder::Asc => (self.first, self.last),
            SortOrder::Desc => (self.last, self.first),
        }
    }
}

fn combines_other_filters(query: &AlbumListQuery) -> bool {
    !query.artist_ids.is_empty()
        || !query.genre_ids.is_empty()
        || query.is_favorite.is_some()
        || query
            .search
            .as_ref()
            .is_some_and(|search| !search.is_empty())
}

fn by_year_params(
    query: &AlbumListQuery,
    range: YearRange,
    start_index: usize,
    size: usize,
) -> QueryParams {
    let (from_year, to_year) = range.bounds_in(query.order);
    let mut params: QueryParams = vec![
        (
            "type",
            SubsonicAlbumListType::ByYear.query_value().to_owned(),
        ),
        ("fromYear", from_year.to_string()),
        ("toYear", to_year.to_string()),
        ("size", size.to_string()),
        ("offset", start_index.to_string()),
    ];
    params.extend(
        query
            .music_folder_id
            .clone()
            .map(|folder| ("musicFolderId", folder)),
    );
    params
}

fn page_size(limit: usize) -> usize {
    match limit {
        0 => SUBSONIC_MAX_ITEMS,
        limit => limit.min(SUBSONIC_MAX_ITEMS),
    }
}

impl NavidromeServer {
    pub(crate) async fn album_page_in_year_range(
        &self,
        query: &AlbumListQuery,
        range: YearRange,
        paging: Paging,
    ) -> Result<Page<Album>> {
        let params = by_year_params(query, range, paging.start_index, page_size(paging.limit));
        let albums = self.albums_by_year(&params, query).await?;
        Ok(subsonic_page(albums, paging.start_index))
    }

    pub(crate) async fn count_albums_in_year_range(
        &self,
        query: &AlbumListQuery,
        range: YearRange,
    ) -> Result<usize> {
        count_by_paging(SUBSONIC_MAX_ITEMS, |start_index| async move {
            let params = by_year_params(query, range, start_index, SUBSONIC_MAX_ITEMS);
            Ok(self.albums_by_year(&params, query).await?.len())
        })
        .await
    }

    async fn albums_by_year(
        &self,
        params: &QueryParams,
        query: &AlbumListQuery,
    ) -> Result<Vec<Album>> {
        if combines_other_filters(query) {
            return Err(ApiError::Unsupported(
                "combining an album year range with other filters",
            ));
        }
        let result: subsonic::AlbumList2 = self.subsonic("getAlbumList2", params).await?;
        Ok(result
            .album_list2
            .unwrap_or_default()
            .album
            .into_iter()
            .map(|album| subsonic::normalize_album(album, self.server_id()))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(year_from: Option<i32>, year_to: Option<i32>, order: SortOrder) -> AlbumListQuery {
        AlbumListQuery {
            year_from,
            year_to,
            order,
            ..AlbumListQuery::default()
        }
    }

    #[test]
    fn a_single_year_is_not_a_range() {
        assert_eq!(
            YearRange::of(&query(Some(2021), Some(2021), SortOrder::Asc)),
            None
        );
        assert_eq!(YearRange::of(&query(None, None, SortOrder::Asc)), None);
    }

    #[test]
    fn open_bounds_reach_the_calendar_limits() {
        let range = YearRange::of(&query(None, Some(1990), SortOrder::Asc));
        assert_eq!(
            range.map(|range| range.bounds_in(SortOrder::Asc)),
            Some((1, 1990))
        );
    }

    #[test]
    fn descending_order_swaps_the_bounds() {
        let descending = query(Some(2020), Some(2022), SortOrder::Desc);
        let range = YearRange::of(&descending).expect("range");
        let params = by_year_params(&descending, range, 10, 5);
        assert!(params.contains(&("fromYear", "2022".into())));
        assert!(params.contains(&("toYear", "2020".into())));
        assert!(params.contains(&("offset", "10".into())));
    }
}
