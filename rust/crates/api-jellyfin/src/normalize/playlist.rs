use feishin_core::model::Playlist;

use super::{non_empty, primary_image, ticks_to_ms};
use crate::dto;

pub fn playlist(item: dto::Playlist, server_id: &str) -> Playlist {
    Playlist {
        server_id: server_id.to_owned(),
        name: item.name.clone(),
        comment: non_empty(item.overview),
        owner: None,
        public: false,
        song_count: item.child_count.unwrap_or(0),
        duration_ms: ticks_to_ms(item.run_time_ticks),
        cover_art_id: primary_image(&item.id, item.image_tags.as_ref()),
        created_at: item.date_created,
        updated_at: None,
        id: item.id,
    }
}
