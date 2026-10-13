use serde::Deserialize;

use super::lenient::{lenient_id, lenient_optional_id};
use super::song::SubsonicSong;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct IndexedArtist {
    #[serde(deserialize_with = "lenient_id")]
    pub id: String,
    pub name: String,
    #[serde(deserialize_with = "lenient_optional_id")]
    pub cover_art: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct FolderIndex {
    pub artist: Option<Vec<IndexedArtist>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Indexes {
    pub index: Option<Vec<FolderIndex>>,
    pub child: Option<Vec<DirectoryChild>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct GetIndexes {
    pub indexes: Option<Indexes>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DirectoryChild {
    pub is_dir: bool,
    pub name: Option<String>,
    #[serde(flatten)]
    pub entry: SubsonicSong,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Directory {
    pub child: Option<Vec<DirectoryChild>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct GetMusicDirectory {
    pub directory: Option<Directory>,
}
