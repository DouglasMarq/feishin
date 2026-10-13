use feishin_core::model::{AlbumListQuery, AlbumSort, SortOrder};

use crate::calendar::current_year;
use crate::client::Params;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlbumListType {
    AlphabeticalByArtist,
    AlphabeticalByName,
    Starred,
    Frequent,
    Random,
    Newest,
    Recent,
    ByYear,
    ByGenre,
}

impl AlbumListType {
    pub fn for_sort(sort: AlbumSort) -> Self {
        match sort {
            AlbumSort::AlbumArtist => Self::AlphabeticalByArtist,
            AlbumSort::Favorited => Self::Starred,
            AlbumSort::PlayCount => Self::Frequent,
            AlbumSort::Random => Self::Random,
            AlbumSort::RecentlyAdded => Self::Newest,
            AlbumSort::RecentlyPlayed => Self::Recent,
            AlbumSort::Year => Self::ByYear,
            AlbumSort::Name | AlbumSort::Rating | AlbumSort::SongCount | AlbumSort::Duration => {
                Self::AlphabeticalByName
            }
        }
    }

    fn as_param(self) -> &'static str {
        match self {
            Self::AlphabeticalByArtist => "alphabeticalByArtist",
            Self::AlphabeticalByName => "alphabeticalByName",
            Self::Starred => "starred",
            Self::Frequent => "frequent",
            Self::Random => "random",
            Self::Newest => "newest",
            Self::Recent => "recent",
            Self::ByYear => "byYear",
            Self::ByGenre => "byGenre",
        }
    }
}

type YearRange = (Option<i32>, Option<i32>);

pub fn nonzero_year_filter(value: Option<i32>) -> Option<i32> {
    value.filter(|year| *year != 0)
}

fn filtered_list_type(
    query: &AlbumListQuery,
    base_type: AlbumListType,
    has_year_filter: bool,
) -> AlbumListType {
    match (has_year_filter, query.genre_ids.is_empty()) {
        (true, _) => AlbumListType::ByYear,
        (false, false) => AlbumListType::ByGenre,
        (false, true) => base_type,
    }
}

fn explicit_year_range(min_year: Option<i32>, max_year: Option<i32>) -> YearRange {
    match (min_year, max_year) {
        (Some(min), Some(max)) => (Some(min), Some(max)),
        (Some(min), None) => (Some(min), Some(current_year())),
        (None, Some(max)) => (Some(0), Some(max)),
        (None, None) => (None, None),
    }
}

fn year_range_for_order(order: SortOrder) -> YearRange {
    match order {
        SortOrder::Asc => (Some(0), Some(current_year())),
        SortOrder::Desc => (Some(current_year()), Some(0)),
    }
}

pub fn album_list_params(query: &AlbumListQuery, base_type: AlbumListType) -> Params {
    let min_year = nonzero_year_filter(query.year_from);
    let max_year = nonzero_year_filter(query.year_to);
    let has_year_filter = min_year.is_some() || max_year.is_some();
    let list_type = filtered_list_type(query, base_type, has_year_filter);
    let (from_year, to_year) = match (list_type, explicit_year_range(min_year, max_year)) {
        (AlbumListType::ByYear, (None, None)) => year_range_for_order(query.order),
        (_, range) => range,
    };

    Params::new()
        .add("type", list_type.as_param())
        .add_optional("genre", query.genre_ids.first())
        .add_optional("fromYear", from_year)
        .add_optional("toYear", to_year)
        .add_optional("musicFolderId", query.music_folder_id.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn year_filter_switches_to_by_year() {
        let query = AlbumListQuery {
            year_to: Some(1990),
            ..Default::default()
        };
        let params = format!("{:?}", album_list_params(&query, AlbumListType::Newest));
        assert!(params.contains("\"byYear\""));
        assert!(params.contains("(\"fromYear\", \"0\")"));
        assert!(params.contains("(\"toYear\", \"1990\")"));
    }

    #[test]
    fn year_sort_without_range_uses_order() {
        let query = AlbumListQuery {
            sort: AlbumSort::Year,
            order: SortOrder::Desc,
            ..Default::default()
        };
        let params = format!(
            "{:?}",
            album_list_params(&query, AlbumListType::for_sort(query.sort))
        );
        assert!(params.contains("(\"toYear\", \"0\")"));
        assert!(params.contains(&format!("(\"fromYear\", \"{}\")", current_year())));
    }
}
