use feishin_core::model::{Genre, MusicFolder};

use crate::dto;

pub fn genre(item: dto::Genre) -> Genre {
    Genre {
        id: item.id,
        name: item.name,
        song_count: item.song_count,
        album_count: item.album_count,
    }
}

pub fn music_folder(item: dto::MusicFolder) -> MusicFolder {
    MusicFolder {
        id: item.id,
        name: item.name,
    }
}
