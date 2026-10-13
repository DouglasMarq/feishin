use feishin_core::model::{AlbumSort, ArtistSort, SongSort, SortOrder};

const SONG_SORT_FALLBACK: &str = "Album,SortName";
const ALBUM_SORT_FALLBACK: &str = "SortName";
const ARTIST_SORT_FALLBACK: &str = "SortName,Name";

pub const SORT_BY_NAME: &str = "SortName";
pub const ALBUM_TRACK_ORDER: &str = "ParentIndexNumber,IndexNumber,SortName";
pub const TOP_SONGS_ORDER: &str = "CommunityRating,SortName";

pub fn sort_order(order: SortOrder) -> &'static str {
    match order {
        SortOrder::Asc => "Ascending",
        SortOrder::Desc => "Descending",
    }
}

pub fn song_sort(sort: SongSort) -> &'static str {
    match sort {
        SongSort::Name => "Name",
        SongSort::Album => "Album,SortName",
        SongSort::AlbumArtist => "AlbumArtist,Album,SortName",
        SongSort::Artist => "Artist,Album,SortName",
        SongSort::Duration => "Runtime,AlbumArtist,Album,SortName",
        SongSort::Year => "PremiereDate,AlbumArtist,Album,SortName",
        SongSort::PlayCount => "PlayCount,SortName",
        SongSort::RecentlyAdded => "DateCreated,SortName",
        SongSort::RecentlyPlayed => "DatePlayed,SortName",
        SongSort::Random => "Random,SortName",
        SongSort::Rating | SongSort::Favorited => SONG_SORT_FALLBACK,
    }
}

pub fn album_sort(sort: AlbumSort) -> &'static str {
    match sort {
        AlbumSort::Name => "SortName",
        AlbumSort::AlbumArtist => "AlbumArtist,SortName",
        AlbumSort::Year => "ProductionYear,PremiereDate,SortName",
        AlbumSort::PlayCount => "PlayCount",
        AlbumSort::RecentlyAdded => "DateCreated,SortName",
        AlbumSort::Random => "Random,SortName",
        AlbumSort::Rating
        | AlbumSort::Favorited
        | AlbumSort::RecentlyPlayed
        | AlbumSort::SongCount
        | AlbumSort::Duration => ALBUM_SORT_FALLBACK,
    }
}

pub fn artist_sort(sort: ArtistSort) -> &'static str {
    match sort {
        ArtistSort::Random => "Random,SortName",
        ArtistSort::Name
        | ArtistSort::AlbumCount
        | ArtistSort::SongCount
        | ArtistSort::PlayCount
        | ArtistSort::Rating
        | ArtistSort::Favorited => ARTIST_SORT_FALLBACK,
    }
}
