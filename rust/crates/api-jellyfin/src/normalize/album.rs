use feishin_core::model::Album;

use super::{artist_refs, credited_artists, names, primary_image, release_year, ticks_to_ms};
use crate::dto;

pub fn album(item: dto::Album, server_id: &str) -> Album {
    let user_data = item.user_data.as_ref();
    Album {
        server_id: server_id.to_owned(),
        name: item.name.clone(),
        album_artists: artist_refs(&item.album_artists),
        artists: credited_artists(&item.artist_items, &item.album_artists),
        year: release_year(item.premiere_date.as_deref(), item.production_year),
        genres: names(&item.genre_items),
        song_count: item.child_count.unwrap_or(0),
        duration_ms: ticks_to_ms(item.run_time_ticks),
        cover_art_id: primary_image(&item.id, item.image_tags.as_ref()),
        play_count: user_data.map_or(0, |data| data.play_count),
        user_rating: None,
        favorite: user_data.is_some_and(|data| data.is_favorite),
        created_at: item.date_created.clone(),
        last_played_at: user_data.and_then(|data| data.last_played_date.clone()),
        id: item.id,
    }
}
