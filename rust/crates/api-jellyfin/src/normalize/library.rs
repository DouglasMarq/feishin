use feishin_core::model::{Folder, Genre, MusicFolder};

use super::primary_image;
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

pub fn folder(item: dto::FolderItem) -> Folder {
    Folder {
        cover_art_id: primary_image(&item.id, item.image_tags.as_ref()),
        id: item.id,
        name: item.name,
    }
}

pub fn library_folder(library: MusicFolder) -> Folder {
    Folder {
        id: library.id,
        name: library.name,
        cover_art_id: None,
    }
}
