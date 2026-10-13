use feishin_core::model::Genre;

use super::catalog::song_genre;
use super::ids::genre_id;
use super::server::FakeServer;
use super::vocabulary::GENRES;

#[derive(Clone, Copy, Default)]
struct GenreTally {
    album_count: u32,
    song_count: u32,
}

impl FakeServer {
    pub(super) fn genres_with_counts(&self) -> Vec<Genre> {
        GENRES
            .iter()
            .zip(self.genre_tallies())
            .enumerate()
            .map(|(genre, (name, tally))| Genre {
                id: genre_id(genre),
                name: (*name).to_owned(),
                song_count: Some(tally.song_count),
                album_count: Some(tally.album_count),
            })
            .collect()
    }

    fn genre_tallies(&self) -> [GenreTally; GENRES.len()] {
        let mut tallies = [GenreTally::default(); GENRES.len()];
        for album in 0..self.total_album_count() {
            let songs = self.album_song_range(album);
            let Some(tally) = tallies.get_mut(song_genre(songs.start)) else {
                continue;
            };
            tally.album_count += 1;
            tally.song_count += songs.len() as u32;
        }
        tallies
    }
}
