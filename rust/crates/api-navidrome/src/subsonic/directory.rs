use feishin_core::model::{Folder, Song};
use serde::Deserialize;

use super::song::{SubsonicSong, normalize_song};
use crate::conversions::non_empty;
use crate::lenient::{optional_string, string, truthy};

#[derive(Debug, Default)]
pub(crate) struct FolderListing {
    pub folders: Vec<Folder>,
    pub songs: Vec<Song>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct IndexedArtist {
    #[serde(deserialize_with = "string")]
    pub id: String,
    #[serde(deserialize_with = "string")]
    pub name: String,
    #[serde(deserialize_with = "optional_string")]
    pub cover_art: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct FolderIndex {
    pub artist: Vec<IndexedArtist>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct DirectoryChild {
    #[serde(deserialize_with = "truthy")]
    pub is_dir: bool,
    #[serde(deserialize_with = "optional_string")]
    pub name: Option<String>,
    #[serde(flatten)]
    pub entry: SubsonicSong,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Indexes {
    pub index: Vec<FolderIndex>,
    pub child: Vec<DirectoryChild>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct IndexesResponse {
    pub indexes: Option<Indexes>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Directory {
    pub child: Vec<DirectoryChild>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct DirectoryResponse {
    pub directory: Option<Directory>,
}

pub(crate) fn indexed_folder(artist: IndexedArtist) -> Folder {
    Folder {
        name: non_empty(Some(artist.name)).unwrap_or_else(|| artist.id.clone()),
        cover_art_id: non_empty(artist.cover_art),
        id: artist.id,
    }
}

fn child_folder(child: DirectoryChild) -> Folder {
    let entry = child.entry;
    let name = non_empty(Some(entry.title))
        .or_else(|| non_empty(child.name))
        .unwrap_or_else(|| entry.id.clone());
    Folder {
        name,
        cover_art_id: non_empty(entry.cover_art),
        id: entry.id,
    }
}

pub(crate) fn folder_listing(children: Vec<DirectoryChild>, server_id: &str) -> FolderListing {
    let mut listing = FolderListing::default();
    for child in children {
        match child.is_dir {
            true => listing.folders.push(child_folder(child)),
            false => listing.songs.push(normalize_song(child.entry, server_id)),
        }
    }
    listing
}
