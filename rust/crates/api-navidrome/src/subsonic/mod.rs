mod album;
mod artist;
mod library;
mod lyrics;
mod playlist;
mod queue;
mod references;
mod search;
mod song;

pub(crate) use album::{AlbumList2, AlbumResponse, normalize_album};
pub(crate) use artist::{ArtistInfo2, ArtistResponse, Artists, normalize_artist};
pub(crate) use library::{Genres, MusicFolders, normalize_genre, normalize_music_folder};
pub(crate) use lyrics::{LyricsList, lyrics_text};
pub(crate) use playlist::{PlaylistResponse, Playlists, SubsonicPlaylist, normalize_playlist};
pub(crate) use queue::{PlayQueue, PlayQueueBody, PlayQueueByIndex};
pub(crate) use search::Search3;
pub(crate) use song::{
    RandomSongs, SimilarSongs, SongResponse, SubsonicSong, TopSongs, normalize_song,
};
