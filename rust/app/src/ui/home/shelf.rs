use feishin_core::model::{Album, AlbumListQuery, AlbumSort, SortOrder};
use slint::Image;

use super::super::HomeAlbum;
use super::super::album_grid::{credited_artist_names, first_credited_artist_id};

pub const SHELF_SIZE: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shelf {
    RecentlyAdded,
    RecentlyPlayed,
    MostPlayed,
    Random,
}

impl Shelf {
    pub const ALL: [Self; 4] = [
        Self::RecentlyAdded,
        Self::RecentlyPlayed,
        Self::MostPlayed,
        Self::Random,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::RecentlyAdded => "Recently added",
            Self::RecentlyPlayed => "Recently played",
            Self::MostPlayed => "Most played",
            Self::Random => "Random",
        }
    }

    pub fn query(self) -> AlbumListQuery {
        let (sort, order) = match self {
            Self::RecentlyAdded => (AlbumSort::RecentlyAdded, SortOrder::Desc),
            Self::RecentlyPlayed => (AlbumSort::RecentlyPlayed, SortOrder::Desc),
            Self::MostPlayed => (AlbumSort::PlayCount, SortOrder::Desc),
            Self::Random => (AlbumSort::Random, SortOrder::Asc),
        };
        AlbumListQuery {
            sort,
            order,
            ..AlbumListQuery::default()
        }
    }

    pub fn reloads_on_every_visit(self) -> bool {
        self != Self::Random
    }

    pub fn is_refreshable(self) -> bool {
        self == Self::Random
    }
}

pub fn album_card(album: &Album) -> HomeAlbum {
    HomeAlbum {
        id: album.id.as_str().into(),
        name: album.name.as_str().into(),
        artist: credited_artist_names(album).into(),
        artist_id: first_credited_artist_id(album).into(),
        cover_id: album.cover_art_id.as_deref().unwrap_or_default().into(),
        cover: Image::default(),
        has_cover: false,
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_albums::album;
    use super::*;

    #[test]
    fn shelves_map_to_feishin_album_queries() {
        let sorts: Vec<_> = Shelf::ALL
            .iter()
            .map(|shelf| (shelf.query().sort, shelf.query().order))
            .collect();
        assert_eq!(
            sorts,
            [
                (AlbumSort::RecentlyAdded, SortOrder::Desc),
                (AlbumSort::RecentlyPlayed, SortOrder::Desc),
                (AlbumSort::PlayCount, SortOrder::Desc),
                (AlbumSort::Random, SortOrder::Asc),
            ]
        );
    }

    #[test]
    fn cards_show_album_artists_and_fall_back_to_track_artists() {
        let card = album_card(&album("a"));
        assert_eq!(
            (
                card.name.as_str(),
                card.artist.as_str(),
                card.artist_id.as_str()
            ),
            ("Name a", "Ada Vale", "artist-1")
        );
        assert_eq!(card.cover_id, "al-a");

        let mut only_artists = album("b");
        only_artists.artists = std::mem::take(&mut only_artists.album_artists);
        assert_eq!(album_card(&only_artists).artist, "Ada Vale");
    }
}
