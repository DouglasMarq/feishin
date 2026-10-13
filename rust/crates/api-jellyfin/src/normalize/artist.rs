use feishin_core::model::Artist;

use super::{non_empty, primary_image};
use crate::dto;

pub fn artist(item: dto::AlbumArtist, server_id: &str) -> Artist {
    Artist {
        server_id: server_id.to_owned(),
        name: item.name.clone(),
        album_count: item.album_count,
        song_count: item.song_count,
        cover_art_id: primary_image(&item.id, item.image_tags.as_ref()),
        favorite: item.user_data.as_ref().is_some_and(|data| data.is_favorite),
        user_rating: None,
        biography: non_empty(item.overview),
        id: item.id,
    }
}
