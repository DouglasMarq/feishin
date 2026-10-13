use serde::Deserialize;

use super::album::SubsonicAlbum;
use super::artist::SubsonicArtist;
use super::library::{SubsonicGenre, SubsonicMusicFolder};
use super::song::SubsonicSong;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct SongContainer {
    pub song: Option<Vec<SubsonicSong>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct SearchResult {
    pub song: Option<Vec<SubsonicSong>>,
    pub album: Option<Vec<SubsonicAlbum>>,
    pub artist: Option<Vec<SubsonicArtist>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Search3 {
    pub search_result3: Option<SearchResult>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Starred2 {
    pub starred2: Option<SearchResult>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct AlbumContainer {
    pub album: Option<Vec<SubsonicAlbum>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AlbumList2 {
    pub album_list2: Option<AlbumContainer>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SongsByGenre {
    pub songs_by_genre: Option<SongContainer>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RandomSongs {
    pub random_songs: Option<SongContainer>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SimilarSongs {
    pub similar_songs: Option<SongContainer>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TopSongs {
    pub top_songs: Option<SongContainer>,
}

#[derive(Debug, Deserialize)]
pub struct GetAlbum {
    pub album: SubsonicAlbum,
}

#[derive(Debug, Deserialize)]
pub struct GetArtist {
    pub artist: SubsonicArtist,
}

#[derive(Debug, Deserialize)]
pub struct GetSong {
    pub song: SubsonicSong,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ArtistIndex {
    pub artist: Option<Vec<SubsonicArtist>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ArtistIndexes {
    pub index: Option<Vec<ArtistIndex>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct GetArtists {
    pub artists: Option<ArtistIndexes>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ArtistInfo {
    pub biography: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ArtistInfo2 {
    pub artist_info2: Option<ArtistInfo>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct GenreContainer {
    pub genre: Option<Vec<SubsonicGenre>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Genres {
    pub genres: Option<GenreContainer>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MusicFolderContainer {
    pub music_folder: Option<Vec<SubsonicMusicFolder>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MusicFolders {
    pub music_folders: Option<MusicFolderContainer>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Empty {}
