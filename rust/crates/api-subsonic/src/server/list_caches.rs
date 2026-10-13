use std::time::Duration;

use feishin_api::TtlCache;
use feishin_core::model::{Album, Playlist, Song, SongListQuery};

use crate::types::SubsonicArtist;

const LIST_TIME_TO_LIVE: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MusicFolderKey(Option<String>);

impl MusicFolderKey {
    pub fn new(music_folder_id: Option<&str>) -> Self {
        Self(music_folder_id.map(str::to_owned))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AllPlaylists;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AlbumAndArtistIds {
    album_ids: Vec<String>,
    artist_ids: Vec<String>,
}

impl AlbumAndArtistIds {
    pub fn of(query: &SongListQuery) -> Self {
        Self {
            album_ids: query.album_ids.clone(),
            artist_ids: query.artist_ids.clone(),
        }
    }
}

pub struct StarredItems {
    pub songs: Vec<Song>,
    pub albums: Vec<Album>,
}

pub struct ListCaches {
    pub artist_index: TtlCache<MusicFolderKey, Vec<SubsonicArtist>>,
    pub starred: TtlCache<MusicFolderKey, StarredItems>,
    pub playlists: TtlCache<AllPlaylists, Vec<Playlist>>,
    pub album_and_artist_songs: TtlCache<AlbumAndArtistIds, Vec<Song>>,
}

impl ListCaches {
    pub fn new() -> Self {
        Self {
            artist_index: TtlCache::new(LIST_TIME_TO_LIVE),
            starred: TtlCache::new(LIST_TIME_TO_LIVE),
            playlists: TtlCache::new(LIST_TIME_TO_LIVE),
            album_and_artist_songs: TtlCache::new(LIST_TIME_TO_LIVE),
        }
    }

    pub fn forget_annotations(&self) {
        self.artist_index.invalidate_all();
        self.starred.invalidate_all();
        self.album_and_artist_songs.invalidate_all();
    }

    pub fn forget_playlists(&self) {
        self.playlists.invalidate_all();
    }
}
