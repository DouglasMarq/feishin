use std::ops::Range;

use super::catalog::first_song_of_album;
use super::hashing::scramble;

pub const DEMO_SONG_COUNT: usize = 20_000;
pub const MAX_DEMO_SONG_COUNT: usize = 5_000_000;
pub(super) const SONGS_PER_ALBUM: usize = 12;
const MIN_ARTIST_COUNT: usize = 400;
const SONGS_PER_ARTIST: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DemoLibrary {
    song_count: usize,
}

impl Default for DemoLibrary {
    fn default() -> Self {
        Self::with_songs(DEMO_SONG_COUNT)
    }
}

impl DemoLibrary {
    pub const fn with_songs(song_count: usize) -> Self {
        Self { song_count }
    }

    pub const fn song_count(self) -> usize {
        self.song_count
    }

    pub const fn album_count(self) -> usize {
        self.song_count.div_ceil(SONGS_PER_ALBUM)
    }

    pub fn artist_count(self) -> usize {
        (self.song_count / SONGS_PER_ARTIST).max(MIN_ARTIST_COUNT)
    }

    pub(super) fn album_artist(self, album: usize) -> usize {
        (scramble(album as u64) % self.artist_count() as u64) as usize
    }

    pub(super) fn album_song_range(self, album: usize) -> Range<usize> {
        let first_song = first_song_of_album(album);
        first_song.min(self.song_count)..(first_song + SONGS_PER_ALBUM).min(self.song_count)
    }

    pub(super) const fn contains_song(self, index: usize) -> bool {
        index < self.song_count
    }

    pub(super) const fn contains_album(self, album: usize) -> bool {
        album < self.album_count()
    }

    pub(super) fn contains_artist(self, artist: usize) -> bool {
        artist < self.artist_count()
    }
}
