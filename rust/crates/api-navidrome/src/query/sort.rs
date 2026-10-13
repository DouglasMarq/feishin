use feishin_core::model::{AlbumSort, ArtistSort, SongSort, SortOrder};

pub(crate) fn order(order: SortOrder) -> &'static str {
    match order {
        SortOrder::Asc => "ASC",
        SortOrder::Desc => "DESC",
    }
}

pub(crate) fn song_sort(sort: SongSort) -> &'static str {
    match sort {
        SongSort::Name => "title",
        SongSort::Album => "album",
        SongSort::AlbumArtist => "order_album_artist_name",
        SongSort::Artist => "artist",
        SongSort::Duration => "duration",
        SongSort::Year => "year",
        SongSort::PlayCount => "playCount",
        SongSort::Rating => "rating",
        SongSort::Favorited => "starred_at",
        SongSort::RecentlyAdded => "createdAt",
        SongSort::RecentlyPlayed => "playDate",
        SongSort::Random => "random",
    }
}

pub(crate) fn album_sort(sort: AlbumSort) -> &'static str {
    match sort {
        AlbumSort::Name => "name",
        AlbumSort::AlbumArtist => "album_artist",
        AlbumSort::Year => "max_year",
        AlbumSort::PlayCount => "play_count",
        AlbumSort::Rating => "rating",
        AlbumSort::Favorited => "starred_at",
        AlbumSort::RecentlyAdded => "recently_added",
        AlbumSort::RecentlyPlayed => "play_date",
        AlbumSort::SongCount => "songCount",
        AlbumSort::Duration => "duration",
        AlbumSort::Random => "random",
    }
}

pub(crate) fn artist_sort(sort: ArtistSort) -> Option<&'static str> {
    match sort {
        ArtistSort::Name => Some("name"),
        ArtistSort::AlbumCount => Some("albumCount"),
        ArtistSort::SongCount => Some("songCount"),
        ArtistSort::PlayCount => Some("playCount"),
        ArtistSort::Rating => Some("rating"),
        ArtistSort::Favorited => Some("starred_at"),
        ArtistSort::Random => None,
    }
}
