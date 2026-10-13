use feishin_core::model::{Artist, Genre, MusicFolder, Playlist};

use super::numbers::{capped_rating, non_empty_text, non_negative_u32, seconds_to_milliseconds};
use crate::types::{SubsonicArtist, SubsonicGenre, SubsonicMusicFolder, SubsonicPlaylist};

pub fn artist(item: SubsonicArtist, server_id: &str) -> Artist {
    Artist {
        id: item.id,
        server_id: server_id.to_owned(),
        name: item.name,
        album_count: Some(non_negative_u32(item.album_count).unwrap_or(0)),
        song_count: None,
        cover_art_id: non_empty_text(item.cover_art),
        favorite: item.starred.is_starred(),
        user_rating: capped_rating(item.user_rating),
        biography: None,
    }
}

pub fn playlist(item: SubsonicPlaylist, server_id: &str) -> Playlist {
    Playlist {
        id: item.id,
        server_id: server_id.to_owned(),
        name: item.name,
        comment: non_empty_text(item.comment),
        owner: non_empty_text(item.owner),
        public: item.public.unwrap_or(false),
        song_count: non_negative_u32(item.song_count).unwrap_or(0),
        duration_ms: seconds_to_milliseconds(item.duration),
        cover_art_id: non_empty_text(item.cover_art),
        created_at: item.created,
        updated_at: item.changed,
    }
}

pub fn genre(item: SubsonicGenre) -> Genre {
    Genre {
        id: item.value.clone(),
        name: item.value,
        song_count: non_negative_u32(item.song_count),
        album_count: non_negative_u32(item.album_count),
    }
}

pub fn music_folder(item: SubsonicMusicFolder) -> MusicFolder {
    MusicFolder {
        id: item.id,
        name: item.name,
    }
}
