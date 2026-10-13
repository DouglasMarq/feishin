use feishin_core::model::{Genre, MusicFolder};
use serde::Deserialize;

use crate::lenient::{optional_u32, string};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct SubsonicGenre {
    #[serde(deserialize_with = "string")]
    pub value: String,
    #[serde(deserialize_with = "optional_u32")]
    pub song_count: Option<u32>,
    #[serde(deserialize_with = "optional_u32")]
    pub album_count: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct SubsonicMusicFolder {
    #[serde(deserialize_with = "string")]
    pub id: String,
    #[serde(deserialize_with = "string")]
    pub name: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct GenreList {
    pub genre: Vec<SubsonicGenre>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Genres {
    pub genres: Option<GenreList>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct FolderList {
    pub music_folder: Vec<SubsonicMusicFolder>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct MusicFolders {
    pub music_folders: Option<FolderList>,
}

pub(crate) fn normalize_genre(genre: SubsonicGenre) -> Genre {
    Genre {
        id: genre.value.clone(),
        name: genre.value,
        song_count: genre.song_count,
        album_count: genre.album_count,
    }
}

pub(crate) fn normalize_music_folder(folder: SubsonicMusicFolder) -> MusicFolder {
    MusicFolder {
        id: folder.id,
        name: folder.name,
    }
}
