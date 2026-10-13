use feishin_core::model::{ArtistListQuery, ArtistSort, SortOrder};

pub const SORTS: [(&str, ArtistSort); 7] = [
    ("Name", ArtistSort::Name),
    ("Album count", ArtistSort::AlbumCount),
    ("Song count", ArtistSort::SongCount),
    ("Play count", ArtistSort::PlayCount),
    ("Rating", ArtistSort::Rating),
    ("Favorited", ArtistSort::Favorited),
    ("Random", ArtistSort::Random),
];

fn sort_at(sort_index: i32) -> ArtistSort {
    usize::try_from(sort_index)
        .ok()
        .and_then(|index| SORTS.get(index))
        .map_or(ArtistSort::Name, |(_, sort)| *sort)
}

fn sort_order(descending: bool) -> SortOrder {
    match descending {
        true => SortOrder::Desc,
        false => SortOrder::Asc,
    }
}

pub fn build_query(
    sort_index: i32,
    descending: bool,
    favorites_only: bool,
    search: &str,
) -> ArtistListQuery {
    let search = search.trim();
    ArtistListQuery {
        sort: sort_at(sort_index),
        order: sort_order(descending),
        search: (!search.is_empty()).then(|| search.to_owned()),
        favorite: favorites_only.then_some(true),
        music_folder_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_follows_the_controls() {
        let query = build_query(2, true, true, "  vale ");
        assert_eq!(query.sort, ArtistSort::SongCount);
        assert_eq!(query.order, SortOrder::Desc);
        assert_eq!(query.search.as_deref(), Some("vale"));
        assert_eq!(query.favorite, Some(true));
        let query = build_query(-1, false, false, " ");
        assert_eq!(query, ArtistListQuery::default());
        assert_eq!(build_query(99, false, false, "").sort, ArtistSort::Name);
    }
}
