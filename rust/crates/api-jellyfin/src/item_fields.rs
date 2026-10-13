pub const SONG_FIELDS: &[&str] = &[
    "Genres",
    "DateCreated",
    "MediaSources",
    "ParentId",
    "Tags",
    "SortName",
    "ProviderIds",
];
pub const ALBUM_LIST_FIELDS: &[&str] =
    &["Tags", "Studios", "SortName", "ProviderIds", "ChildCount"];
pub const ALBUM_DETAIL_FIELDS: &[&str] =
    &["Genres", "DateCreated", "ChildCount", "Tags", "ProviderIds"];
pub const ARTIST_LIST_FIELDS: &[&str] = &[
    "Genres",
    "DateCreated",
    "ExternalUrls",
    "Overview",
    "SortName",
    "ProviderIds",
    "ItemCounts",
];
pub const ARTIST_DETAIL_FIELDS: &[&str] = &["Genres", "Overview", "SortName", "ItemCounts"];
pub const PLAYLIST_LIST_FIELDS: &[&str] = &[
    "ChildCount",
    "Genres",
    "DateCreated",
    "ParentId",
    "Overview",
];
pub const PLAYLIST_DETAIL_FIELDS: &[&str] = &[
    "Genres",
    "DateCreated",
    "MediaSources",
    "ChildCount",
    "ParentId",
    "SortName",
];
pub const GENRE_FIELDS: &[&str] = &["ItemCounts"];
pub const NO_FIELDS: &[&str] = &[];

pub const SONG_ITEM_TYPE: &str = "Audio";
pub const ALBUM_ITEM_TYPE: &str = "MusicAlbum";
pub const PLAYLIST_ITEM_TYPE: &str = "Playlist";
