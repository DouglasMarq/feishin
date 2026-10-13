use serde::{Deserialize, Serialize};

use super::album::Album;
use super::artist::Artist;
use super::song::Song;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResults {
    pub songs: Vec<Song>,
    pub albums: Vec<Album>,
    pub artists: Vec<Artist>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemKind {
    Song,
    Album,
    Artist,
    Playlist,
}
