use feishin_core::model::{Folder, Share, Song};

use super::numbers::non_empty_text;
use super::song::song;
use crate::types::{DirectoryChild, IndexedArtist, SubsonicShare};

#[derive(Debug, Default)]
pub struct FolderListing {
    pub folders: Vec<Folder>,
    pub songs: Vec<Song>,
}

pub fn indexed_folder(artist: IndexedArtist) -> Folder {
    Folder {
        name: non_empty_text(Some(artist.name)).unwrap_or_else(|| artist.id.clone()),
        cover_art_id: non_empty_text(artist.cover_art),
        id: artist.id,
    }
}

fn child_folder(child: DirectoryChild) -> Folder {
    let entry = child.entry;
    let name = non_empty_text(Some(entry.title))
        .or_else(|| non_empty_text(child.name))
        .unwrap_or_else(|| entry.id.clone());
    Folder {
        name,
        cover_art_id: non_empty_text(entry.cover_art),
        id: entry.id,
    }
}

pub fn folder_listing(children: Vec<DirectoryChild>, server_id: &str) -> FolderListing {
    let mut listing = FolderListing::default();
    for child in children {
        match child.is_dir {
            true => listing.folders.push(child_folder(child)),
            false => listing.songs.push(song(child.entry, server_id)),
        }
    }
    listing
}

pub fn share(item: SubsonicShare) -> Share {
    Share {
        id: item.id,
        url: item.url,
        description: non_empty_text(item.description),
        expires_at: non_empty_text(item.expires),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn child(value: serde_json::Value) -> DirectoryChild {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn children_split_into_folders_and_songs() {
        let children = vec![
            child(json!({ "id": 10, "isDir": true, "title": "Arrival", "coverArt": "c10" })),
            child(json!({ "id": "s1", "isDir": false, "title": "Dancing Queen", "duration": 231 })),
            child(json!({ "id": "d2", "isDir": true, "name": "Loose" })),
        ];
        let listing = folder_listing(children, "srv");
        let folder_names: Vec<&str> = listing
            .folders
            .iter()
            .map(|folder| folder.name.as_str())
            .collect();
        assert_eq!(folder_names, ["Arrival", "Loose"]);
        assert_eq!(listing.folders[0].id, "10");
        assert_eq!(listing.folders[0].cover_art_id.as_deref(), Some("c10"));
        assert_eq!(listing.songs.len(), 1);
        assert_eq!(listing.songs[0].duration_ms, 231_000);
    }
}
