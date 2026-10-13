use std::time::Duration;

use feishin_api::TtlCache;
use feishin_core::model::{Album, Playlist, Song};

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

pub struct StarredItems {
    pub songs: Vec<Song>,
    pub albums: Vec<Album>,
}

pub struct ListCaches {
    pub artist_index: TtlCache<MusicFolderKey, Vec<SubsonicArtist>>,
    pub starred: TtlCache<MusicFolderKey, StarredItems>,
    pub playlists: TtlCache<AllPlaylists, Vec<Playlist>>,
}

impl ListCaches {
    pub fn new() -> Self {
        Self {
            artist_index: TtlCache::new(LIST_TIME_TO_LIVE),
            starred: TtlCache::new(LIST_TIME_TO_LIVE),
            playlists: TtlCache::new(LIST_TIME_TO_LIVE),
        }
    }

    pub fn forget_annotations(&self) {
        self.artist_index.invalidate_all();
        self.starred.invalidate_all();
    }

    pub fn forget_playlists(&self) {
        self.playlists.invalidate_all();
    }
}
