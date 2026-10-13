use feishin_core::model::AlbumSort;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SubsonicAlbumListType {
    AlphabeticalByName,
    AlphabeticalByArtist,
    Starred,
    Frequent,
    Random,
    Newest,
    Recent,
    ByYear,
}

impl SubsonicAlbumListType {
    pub(crate) fn for_sort(sort: AlbumSort) -> Option<Self> {
        match sort {
            AlbumSort::Name => Some(Self::AlphabeticalByName),
            AlbumSort::AlbumArtist => Some(Self::AlphabeticalByArtist),
            AlbumSort::Favorited => Some(Self::Starred),
            AlbumSort::PlayCount => Some(Self::Frequent),
            AlbumSort::Random => Some(Self::Random),
            AlbumSort::RecentlyAdded => Some(Self::Newest),
            AlbumSort::RecentlyPlayed => Some(Self::Recent),
            AlbumSort::Year => Some(Self::ByYear),
            AlbumSort::Rating | AlbumSort::SongCount | AlbumSort::Duration => None,
        }
    }

    pub(crate) const fn query_value(self) -> &'static str {
        match self {
            Self::AlphabeticalByName => "alphabeticalByName",
            Self::AlphabeticalByArtist => "alphabeticalByArtist",
            Self::Starred => "starred",
            Self::Frequent => "frequent",
            Self::Random => "random",
            Self::Newest => "newest",
            Self::Recent => "recent",
            Self::ByYear => "byYear",
        }
    }
}
