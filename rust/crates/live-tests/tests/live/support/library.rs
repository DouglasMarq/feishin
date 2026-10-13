use feishin_api::MusicServer;
use feishin_core::model::{
    Album, AlbumListQuery, Artist, ArtistListQuery, Paging, Song, SongListQuery,
};

pub const SONG_COUNT: usize = 8;
pub const ALBUM_COUNT: usize = 4;
pub const ARTIST_NAMES: [&str; 3] = ["Aurora Tones", "Bravo Sine", "Charlie Square"];
pub const GENRE_NAMES: [&str; 3] = ["Electronic", "Jazz", "Rock"];
pub const ALBUM_NAMES: [&str; ALBUM_COUNT] =
    ["Afterglow", "First Light", "Second Wave", "Third Signal"];
pub const SONG_TITLES: [&str; SONG_COUNT] = [
    "Blue Note",
    "Dawn Chorus",
    "Dusk Signal",
    "Late Set",
    "Morning Haze",
    "Noon Glare",
    "Tidal Pull",
    "Undertow",
];
pub const SONG_TITLES_BY_DURATION: [&str; SONG_COUNT] = [
    "Dawn Chorus",
    "Morning Haze",
    "Noon Glare",
    "Tidal Pull",
    "Dusk Signal",
    "Undertow",
    "Blue Note",
    "Late Set",
];
pub const COVER_ALBUM: &str = "First Light";
pub const LYRICS_SONG: &str = "Tidal Pull";
pub const LYRICS_LINE: &str = "Pulling at the shore";

pub fn everything() -> Paging {
    Paging {
        start_index: 0,
        limit: 100,
    }
}

pub fn titles(songs: &[Song]) -> Vec<&str> {
    songs.iter().map(|song| song.title.as_str()).collect()
}

pub fn album_names(albums: &[Album]) -> Vec<&str> {
    albums.iter().map(|album| album.name.as_str()).collect()
}

pub fn artist_names(artists: &[Artist]) -> Vec<&str> {
    artists.iter().map(|artist| artist.name.as_str()).collect()
}

pub async fn all_songs(server: &dyn MusicServer) -> Vec<Song> {
    server
        .song_list(&SongListQuery::default(), everything())
        .await
        .expect("song list")
        .items
}

pub async fn song_titled(server: &dyn MusicServer, title: &str) -> Song {
    all_songs(server)
        .await
        .into_iter()
        .find(|song| song.title == title)
        .unwrap_or_else(|| panic!("song {title} is missing"))
}

pub async fn album_named(server: &dyn MusicServer, name: &str) -> Album {
    server
        .album_list(&AlbumListQuery::default(), everything())
        .await
        .expect("album list")
        .items
        .into_iter()
        .find(|album| album.name == name)
        .unwrap_or_else(|| panic!("album {name} is missing"))
}

pub async fn artist_named(server: &dyn MusicServer, name: &str) -> Artist {
    server
        .artist_list(&ArtistListQuery::default(), everything())
        .await
        .expect("artist list")
        .items
        .into_iter()
        .find(|artist| artist.name == name)
        .unwrap_or_else(|| panic!("artist {name} is missing"))
}

pub async fn genre_id_named(server: &dyn MusicServer, name: &str) -> String {
    server
        .genre_list()
        .await
        .expect("genres")
        .into_iter()
        .find(|genre| genre.name == name)
        .unwrap_or_else(|| panic!("genre {name} is missing"))
        .id
}
