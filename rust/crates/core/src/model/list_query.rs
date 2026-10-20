use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SortOrder {
    #[default]
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SongSort {
    #[default]
    Name,
    Album,
    AlbumArtist,
    Artist,
    Duration,
    Year,
    PlayCount,
    Rating,
    Favorited,
    RecentlyAdded,
    RecentlyPlayed,
    Random,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SongListQuery {
    pub sort: SongSort,
    pub order: SortOrder,
    pub search: Option<String>,
    #[serde(rename = "favorite")]
    pub is_favorite: Option<bool>,
    pub genre_ids: Vec<String>,
    pub artist_ids: Vec<String>,
    pub album_ids: Vec<String>,
    pub year_from: Option<i32>,
    pub year_to: Option<i32>,
    pub music_folder_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AlbumSort {
    #[default]
    Name,
    AlbumArtist,
    Year,
    PlayCount,
    Rating,
    Favorited,
    RecentlyAdded,
    RecentlyPlayed,
    SongCount,
    Duration,
    Random,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AlbumListQuery {
    pub sort: AlbumSort,
    pub order: SortOrder,
    pub search: Option<String>,
    #[serde(rename = "favorite")]
    pub is_favorite: Option<bool>,
    pub genre_ids: Vec<String>,
    pub artist_ids: Vec<String>,
    pub year_from: Option<i32>,
    pub year_to: Option<i32>,
    pub music_folder_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ArtistSort {
    #[default]
    Name,
    AlbumCount,
    SongCount,
    PlayCount,
    Rating,
    Favorited,
    Random,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ArtistListQuery {
    pub sort: ArtistSort,
    pub order: SortOrder,
    pub search: Option<String>,
    #[serde(rename = "favorite")]
    pub is_favorite: Option<bool>,
    pub music_folder_id: Option<String>,
}
