use feishin_api::{ApiError, Result};
use feishin_core::model::{Folder, FolderId, Page, Paging, Song};

use super::catalog::{album_name, artist_name};
use super::ids::{album_cover_id, artist_cover_id};
use super::paging::paginate;
use super::server::FakeServer;

const ARTIST_FOLDER_PREFIX: &str = "folder-artist-";
const ALBUM_FOLDER_PREFIX: &str = "folder-album-";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DemoFolder {
    Root,
    Artist(usize),
    Album(usize),
}

pub(super) fn artist_folder_id(artist: usize) -> String {
    format!("{ARTIST_FOLDER_PREFIX}{artist}")
}

pub(super) fn album_folder_id(album: usize) -> String {
    format!("{ALBUM_FOLDER_PREFIX}{album}")
}

fn artist_folder(artist: usize) -> Folder {
    Folder {
        id: artist_folder_id(artist),
        name: artist_name(artist),
        cover_art_id: Some(artist_cover_id(artist)),
    }
}

fn album_folder(album: usize) -> Folder {
    Folder {
        id: album_folder_id(album),
        name: album_name(album),
        cover_art_id: Some(album_cover_id(album)),
    }
}

fn parse_directory(id: &str) -> Option<DemoFolder> {
    let index_after = |prefix: &str| id.strip_prefix(prefix)?.parse::<usize>().ok();
    index_after(ARTIST_FOLDER_PREFIX)
        .map(DemoFolder::Artist)
        .or_else(|| index_after(ALBUM_FOLDER_PREFIX).map(DemoFolder::Album))
}

impl FakeServer {
    pub(super) fn demo_subfolders(&self, folder: &FolderId) -> Result<Vec<Folder>> {
        Ok(match self.demo_folder(folder)? {
            DemoFolder::Root => (0..self.library.artist_count())
                .map(artist_folder)
                .collect(),
            DemoFolder::Artist(artist) => self
                .artist_albums
                .albums_of(artist)
                .iter()
                .map(|album| album_folder(*album))
                .collect(),
            DemoFolder::Album(_) => Vec::new(),
        })
    }

    pub(super) fn demo_folder_song_page(
        &self,
        folder: &FolderId,
        paging: Paging,
    ) -> Result<Page<Song>> {
        let songs = match self.demo_folder(folder)? {
            DemoFolder::Album(album) => self.album_song_range(album),
            DemoFolder::Root | DemoFolder::Artist(_) => 0..0,
        };
        Ok(paginate(songs.map(|index| self.song_at(index)), paging))
    }

    fn demo_folder(&self, folder: &FolderId) -> Result<DemoFolder> {
        let FolderId::Directory(id) = folder else {
            return Ok(DemoFolder::Root);
        };
        match parse_directory(id) {
            Some(DemoFolder::Artist(artist)) if self.library.contains_artist(artist) => {
                Ok(DemoFolder::Artist(artist))
            }
            Some(DemoFolder::Album(album)) if self.library.contains_album(album) => {
                Ok(DemoFolder::Album(album))
            }
            _ => Err(ApiError::NotFound),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paging(start_index: usize, limit: usize) -> Paging {
        Paging { start_index, limit }
    }

    #[test]
    fn the_root_holds_one_folder_per_artist() {
        let server = FakeServer::new(2_000);
        let folders = server.demo_subfolders(&FolderId::Root).unwrap();
        assert_eq!(folders.len(), server.library.artist_count());
        assert_eq!(folders[3].id, "folder-artist-3");
        let songs = server
            .demo_folder_song_page(&FolderId::Root, paging(0, 10))
            .unwrap();
        assert_eq!(songs.total_count, Some(0));
    }

    #[test]
    fn artist_folders_hold_their_albums_and_album_folders_their_songs() {
        let server = FakeServer::new(2_000);
        let artist = server.library.album_artist(0);
        let albums = server
            .demo_subfolders(&FolderId::directory(artist_folder_id(artist)))
            .unwrap();
        assert!(albums.iter().any(|folder| folder.id == "folder-album-0"));

        let album = FolderId::directory(album_folder_id(0));
        assert!(server.demo_subfolders(&album).unwrap().is_empty());
        let songs = server.demo_folder_song_page(&album, paging(10, 5)).unwrap();
        assert_eq!(songs.total_count, Some(12));
        let ids: Vec<&str> = songs.items.iter().map(|song| song.id.as_str()).collect();
        assert_eq!(ids, ["song-10", "song-11"]);
    }

    #[test]
    fn unknown_folders_are_not_found() {
        let server = FakeServer::new(100);
        for id in ["folder-album-999", "folder-artist-99999", "elsewhere"] {
            let folder = FolderId::directory(id);
            assert!(matches!(
                server.demo_subfolders(&folder),
                Err(ApiError::NotFound)
            ));
        }
    }
}
