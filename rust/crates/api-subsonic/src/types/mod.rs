mod album;
mod artist;
mod lenient;
mod library;
mod lyrics;
mod play_queue;
mod playlist;
mod radio;
mod responses;
mod scan_status;
mod server_info;
mod song;
mod starred;

pub use album::{ReleaseDate, SubsonicAlbum};
pub use artist::SubsonicArtist;
pub use library::{SubsonicGenre, SubsonicMusicFolder};
pub use lyrics::{LyricsList, StructuredLyric};
pub use play_queue::{GetPlayQueue, GetPlayQueueByIndex, PlayQueue};
pub use playlist::{PlaylistResponse, Playlists, SubsonicPlaylist};
pub use radio::{InternetRadioStations, SubsonicRadioStation, UserResponse};
pub use responses::{
    AlbumList2, ArtistInfo2, Empty, Genres, GetAlbum, GetArtist, GetArtists, GetSong, MusicFolders,
    RandomSongs, Search3, SearchResult, SimilarSongs, SongsByGenre, Starred2, TopSongs,
};
pub use scan_status::{ScanStatus, ScanStatusResponse};
pub use server_info::{OpenSubsonicExtensions, Ping};
pub use song::{Contributor, GenreItem, ReplayGain, SimpleArtist, SubsonicSong};

#[cfg(test)]
pub use lyrics::LyricLine;
