use feishin_core::model::{Album, ArtistRef};
use slint::Image;

pub fn album(id: &str) -> Album {
    Album {
        id: id.to_owned(),
        server_id: "demo".into(),
        name: format!("Name {id}"),
        album_artists: vec![ArtistRef {
            id: "artist-1".into(),
            name: "Ada Vale".into(),
        }],
        artists: Vec::new(),
        year: None,
        genres: Vec::new(),
        song_count: 1,
        duration_ms: 0,
        cover_art_id: Some(format!("al-{id}")),
        play_count: 0,
        user_rating: None,
        favorite: false,
        created_at: None,
        last_played_at: None,
    }
}

pub fn albums(ids: &[&str]) -> Vec<Album> {
    ids.iter().map(|id| album(id)).collect()
}

pub fn one_pixel_image() -> Image {
    Image::from_rgba8(slint::SharedPixelBuffer::new(1, 1))
}
