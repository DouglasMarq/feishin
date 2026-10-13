use std::sync::MutexGuard;
use std::sync::PoisonError;

use feishin_api::{ApiError, Result};
use feishin_core::model::{Page, Paging, Playlist, Song};

use super::catalog::song_duration;
use super::hashing::scramble;
use super::ids::playlist_cover_id;
use super::paging::paginate;
use super::server::FakeServer;
use super::vocabulary::PLAYLIST_NAMES;

pub(super) struct DemoPlaylist {
    id: String,
    name: String,
    comment: Option<String>,
    songs: Vec<usize>,
}

#[derive(Default)]
pub(super) struct DemoPlaylists {
    items: Vec<DemoPlaylist>,
    next_id: usize,
}

impl DemoPlaylists {
    pub(super) fn generated(song_count: usize) -> Self {
        let mut playlists = Self::default();
        if song_count == 0 {
            return playlists;
        }
        for (position, name) in PLAYLIST_NAMES.iter().enumerate() {
            let length = 8 + (scramble(position as u64 ^ 0x9e7) % 60) as usize;
            let songs = generated_playlist_songs(position, length, song_count);
            let comment =
                (position % 2 == 0).then(|| format!("A demo playlist with {length} songs"));
            let playlist = playlists.allocate(name, comment, songs);
            playlists.items.push(playlist);
        }
        playlists
    }

    fn allocate(&mut self, name: &str, comment: Option<String>, songs: Vec<usize>) -> DemoPlaylist {
        self.next_id += 1;
        DemoPlaylist {
            id: format!("playlist-{}", self.next_id),
            name: name.to_owned(),
            comment,
            songs,
        }
    }

    fn get_mut(&mut self, id: &str) -> Result<&mut DemoPlaylist> {
        self.items
            .iter_mut()
            .find(|playlist| playlist.id == id)
            .ok_or(ApiError::NotFound)
    }
}

fn generated_playlist_songs(position: usize, length: usize, song_count: usize) -> Vec<usize> {
    (0..length)
        .map(|offset| {
            (scramble((position * 1000 + offset) as u64 ^ 0x91) % song_count as u64) as usize
        })
        .collect()
}

impl FakeServer {
    fn locked_playlists(&self) -> MutexGuard<'_, DemoPlaylists> {
        self.playlists
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn to_playlist(&self, playlist: &DemoPlaylist) -> Playlist {
        Playlist {
            id: playlist.id.clone(),
            server_id: self.config.id.clone(),
            name: playlist.name.clone(),
            comment: playlist.comment.clone(),
            owner: Some(self.config.username.clone()),
            public: false,
            song_count: playlist.songs.len() as u32,
            duration_ms: playlist
                .songs
                .iter()
                .map(|&index| song_duration(index))
                .sum(),
            cover_art_id: Some(playlist_cover_id(&playlist.id)),
            created_at: None,
            updated_at: None,
        }
    }

    pub(super) fn playlist_page(&self, paging: Paging) -> Page<Playlist> {
        let playlists = self.locked_playlists();
        paginate(
            playlists
                .items
                .iter()
                .map(|playlist| self.to_playlist(playlist)),
            paging,
        )
    }

    pub(super) fn songs_of_playlist(&self, id: &str) -> Result<Vec<Song>> {
        let mut playlists = self.locked_playlists();
        let playlist = playlists.get_mut(id)?;
        Ok(playlist
            .songs
            .iter()
            .map(|&index| self.song_at(index))
            .collect())
    }

    pub(super) fn insert_playlist(&self, name: &str, song_ids: &[String]) -> Playlist {
        let songs = self.song_indexes(song_ids);
        let mut playlists = self.locked_playlists();
        let playlist = playlists.allocate(name, None, songs);
        let summary = self.to_playlist(&playlist);
        playlists.items.push(playlist);
        summary
    }

    pub(super) fn append_to_playlist(&self, playlist_id: &str, song_ids: &[String]) -> Result<()> {
        let songs = self.song_indexes(song_ids);
        let mut playlists = self.locked_playlists();
        playlists.get_mut(playlist_id)?.songs.extend(songs);
        Ok(())
    }

    pub(super) fn remove_playlist_positions(
        &self,
        playlist_id: &str,
        positions: &[usize],
    ) -> Result<()> {
        let mut playlists = self.locked_playlists();
        let playlist = playlists.get_mut(playlist_id)?;
        playlist.songs = std::mem::take(&mut playlist.songs)
            .into_iter()
            .enumerate()
            .filter(|(position, _)| !positions.contains(position))
            .map(|(_, song)| song)
            .collect();
        Ok(())
    }

    pub(super) fn remove_playlist(&self, id: &str) -> Result<()> {
        let mut playlists = self.locked_playlists();
        let count_before = playlists.items.len();
        playlists.items.retain(|playlist| playlist.id != id);
        if playlists.items.len() == count_before {
            return Err(ApiError::NotFound);
        }
        Ok(())
    }
}
