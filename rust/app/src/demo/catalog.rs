use feishin_core::model::ArtistRef;

use super::hashing::{pick_word, scramble};
use super::ids::artist_id;
use super::vocabulary::{ADJECTIVES, FIRST_NAMES, GENRES, LAST_NAMES, NOUNS};

pub const DEMO_SONG_COUNT: usize = 20_000;
pub(super) const SONGS_PER_ALBUM: usize = 12;
pub(super) const ARTIST_COUNT: usize = 400;

pub(super) fn album_of_song(index: usize) -> usize {
    index / SONGS_PER_ALBUM
}

pub(super) fn first_song_of_album(album: usize) -> usize {
    album * SONGS_PER_ALBUM
}

pub(super) fn artist_name(artist: usize) -> String {
    let hash = scramble(artist as u64 ^ 0xa11);
    format!(
        "{} {}",
        pick_word(&FIRST_NAMES, hash),
        pick_word(&LAST_NAMES, hash >> 16)
    )
}

pub(super) fn artist_ref(artist: usize) -> ArtistRef {
    ArtistRef {
        id: artist_id(artist),
        name: artist_name(artist),
    }
}

pub(super) fn album_name(album: usize) -> String {
    let hash = scramble(album as u64 ^ 0xa1b);
    format!(
        "{} {}",
        pick_word(&ADJECTIVES, hash),
        pick_word(&NOUNS, hash >> 16)
    )
}

pub(super) fn album_artist(album: usize) -> usize {
    (scramble(album as u64) % ARTIST_COUNT as u64) as usize
}

pub(super) fn album_year(album: usize) -> i32 {
    1965 + (scramble(album as u64 ^ 0x7ea) % 60) as i32
}

pub(super) fn album_is_favorite(album: usize) -> bool {
    scramble(album as u64 ^ 0xfa7).is_multiple_of(6)
}

pub(super) fn song_title(index: usize) -> String {
    let hash = scramble(index as u64);
    let title = format!(
        "{} {}",
        pick_word(&ADJECTIVES, hash),
        pick_word(&NOUNS, hash >> 16)
    );
    let tail = pick_word(&NOUNS, hash >> 40);
    match (hash >> 32) % 6 {
        0 => format!("{title} (Live)"),
        1 => format!("The {title}"),
        2 => format!("{title} of the {tail}"),
        3 => format!("{title} {tail}"),
        _ => title,
    }
}

pub(super) fn song_duration(index: usize) -> u64 {
    90_000 + (scramble(index as u64 ^ 0xd0) % 330) * 1000
}

pub(super) fn song_genre(index: usize) -> usize {
    (scramble(album_of_song(index) as u64 ^ 0x6e) % GENRES.len() as u64) as usize
}

pub(super) fn genre_name(genre: usize) -> &'static str {
    GENRES.get(genre).copied().unwrap_or_default()
}

pub(super) fn song_traits_hash(index: usize) -> u64 {
    scramble(index as u64 ^ 0x5eed)
}

pub(super) fn song_is_favorite(index: usize) -> bool {
    song_traits_hash(index).is_multiple_of(9)
}

pub(super) fn rating_from_hash(hash: u64) -> Option<u8> {
    Some((hash >> 8) as u8 % 6).filter(|rating| *rating > 0)
}
