const SONG_PREFIX: &str = "song-";
const ALBUM_PREFIX: &str = "album-";
const ARTIST_PREFIX: &str = "artist-";

pub(super) fn song_id(index: usize) -> String {
    format!("{SONG_PREFIX}{index}")
}

pub(super) fn album_id(album: usize) -> String {
    format!("{ALBUM_PREFIX}{album}")
}

pub(super) fn artist_id(artist: usize) -> String {
    format!("{ARTIST_PREFIX}{artist}")
}

pub(super) fn genre_id(genre: usize) -> String {
    format!("genre-{genre}")
}

pub(super) fn album_cover_id(album: usize) -> String {
    format!("al-{album}")
}

pub(super) fn artist_cover_id(artist: usize) -> String {
    format!("ar-{artist}")
}

pub(super) fn playlist_cover_id(playlist_id: &str) -> String {
    format!("pl-{playlist_id}")
}

pub(super) fn parse_song_index(id: &str) -> Option<usize> {
    parse_index(id, SONG_PREFIX)
}

pub(super) fn parse_album_index(id: &str) -> Option<usize> {
    parse_index(id, ALBUM_PREFIX)
}

pub(super) fn parse_artist_index(id: &str) -> Option<usize> {
    parse_index(id, ARTIST_PREFIX)
}

fn parse_index(id: &str, prefix: &str) -> Option<usize> {
    id.strip_prefix(prefix)?.parse().ok()
}
