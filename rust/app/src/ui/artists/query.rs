use feishin_core::model::{ArtistListQuery, ArtistSort};

use crate::ui::sort_order;

pub const SORTS: [ArtistSort; 7] = [
    ArtistSort::Name,
    ArtistSort::AlbumCount,
    ArtistSort::SongCount,
    ArtistSort::PlayCount,
    ArtistSort::Rating,
    ArtistSort::Favorited,
    ArtistSort::Random,
];

pub fn sort_label(sort: ArtistSort) -> String {
    match sort {
        ArtistSort::Name => tr!("Name"),
        ArtistSort::AlbumCount => tr!("Album count"),
        ArtistSort::SongCount => tr!("Song count"),
        ArtistSort::PlayCount => tr!("Play count"),
        ArtistSort::Rating => tr!("Rating"),
        ArtistSort::Favorited => tr!("Favorited"),
        ArtistSort::Random => tr!("Random"),
    }
}

fn sort_at(sort_index: i32) -> ArtistSort {
    usize::try_from(sort_index)
        .ok()
        .and_then(|index| SORTS.get(index))
        .copied()
        .unwrap_or(ArtistSort::Name)
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
    use feishin_core::model::SortOrder;

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
