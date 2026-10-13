use std::ops::Range;

use feishin_core::model::{Album, Artist, Song};

use super::catalog::{
    SONGS_PER_ALBUM, album_artist, album_is_favorite, album_name, album_of_song, album_year,
    artist_name, artist_ref, first_song_of_album, genre_name, rating_from_hash, song_duration,
    song_genre, song_title, song_traits_hash,
};
use super::hashing::{pick_word, scramble};
use super::ids::{album_cover_id, album_id, artist_cover_id, artist_id, parse_song_index, song_id};
use super::server::FakeServer;
use super::songs::blank_song;
use super::vocabulary::GENRES;

impl FakeServer {
    pub(super) fn total_album_count(&self) -> usize {
        self.song_count.div_ceil(SONGS_PER_ALBUM)
    }

    pub(super) fn album_song_range(&self, album: usize) -> Range<usize> {
        let first_song = first_song_of_album(album);
        first_song..(first_song + SONGS_PER_ALBUM).min(self.song_count)
    }

    pub(super) fn song_index_in_library(&self, song_id: &str) -> Option<usize> {
        parse_song_index(song_id).filter(|&index| index < self.song_count)
    }

    pub(super) fn song_indexes(&self, song_ids: &[String]) -> Vec<usize> {
        song_ids
            .iter()
            .filter_map(|song_id| self.song_index_in_library(song_id))
            .collect()
    }

    pub fn song_at(&self, index: usize) -> Song {
        let album = album_of_song(index);
        let artist = album_artist(album);
        let hash = song_traits_hash(index);
        Song {
            title: song_title(index),
            artists: vec![artist_ref(artist)],
            album_artists: vec![artist_ref(artist)],
            album: Some(album_name(album)),
            album_id: Some(album_id(album)),
            track_number: Some((index % SONGS_PER_ALBUM) as u32 + 1),
            disc_number: Some(1),
            year: Some(album_year(album)),
            genres: vec![genre_name(song_genre(index)).to_owned()],
            bitrate_kbps: Some(320),
            play_count: (hash % 60) as u32,
            user_rating: rating_from_hash(hash),
            favorite: hash.is_multiple_of(9),
            cover_art_id: Some(album_cover_id(album)),
            container: Some("flac".into()),
            created_at: Some(format!("2024-01-01T00:00:{:02}Z", index % 60)),
            ..blank_song(&song_id(index), &self.config.id, song_duration(index))
        }
    }

    pub(super) fn album_at(&self, album: usize) -> Album {
        let songs = self.album_song_range(album);
        let artist = album_artist(album);
        Album {
            id: album_id(album),
            server_id: self.config.id.clone(),
            name: album_name(album),
            album_artists: vec![artist_ref(artist)],
            artists: vec![artist_ref(artist)],
            year: Some(album_year(album)),
            genres: vec![genre_name(song_genre(songs.start)).to_owned()],
            song_count: songs.len() as u32,
            duration_ms: songs.map(song_duration).sum(),
            cover_art_id: Some(album_cover_id(album)),
            play_count: 0,
            user_rating: None,
            favorite: album_is_favorite(album),
            created_at: None,
            last_played_at: None,
        }
    }

    pub(super) fn artist_albums(&self, artist: usize) -> impl Iterator<Item = usize> {
        (0..self.total_album_count()).filter(move |&album| album_artist(album) == artist)
    }

    pub(super) fn artist_at(&self, artist: usize) -> Artist {
        let albums: Vec<usize> = self.artist_albums(artist).collect();
        let songs = albums
            .iter()
            .map(|&album| self.album_at(album).song_count)
            .sum::<u32>();
        let hash = scramble(artist as u64 ^ 0xb10);
        let name = artist_name(artist);
        Artist {
            id: artist_id(artist),
            server_id: self.config.id.clone(),
            album_count: Some(albums.len() as u32),
            song_count: Some(songs),
            cover_art_id: Some(artist_cover_id(artist)),
            favorite: hash.is_multiple_of(5),
            user_rating: rating_from_hash(hash),
            biography: Some(artist_biography(&name, hash, &albums)),
            name,
        }
    }
}

fn artist_biography(name: &str, hash: u64, albums: &[usize]) -> String {
    let genre = pick_word(&GENRES, hash).to_lowercase();
    let first_album = albums
        .first()
        .map_or_else(|| "nothing yet".to_owned(), |&album| album_name(album));
    format!(
        "<p>{name} is a {genre} artist known for <i>{first_album}</i>.</p><p>Over {} albums, \
         {name} moved from small &amp; noisy rooms to large stages. <a \
         href=\"https://www.last.fm\">Read more on Last.fm</a></p>",
        albums.len()
    )
}
