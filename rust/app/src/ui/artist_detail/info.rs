use feishin_core::model::{Album, ArtistDetail};
use slint::Image;

use super::super::ArtistAlbumCard;
use super::super::artists::count_label;

fn album_count(detail: &ArtistDetail) -> Option<u32> {
    detail
        .artist
        .album_count
        .or_else(|| u32::try_from(detail.albums.len()).ok())
}

fn song_count(detail: &ArtistDetail) -> Option<u32> {
    detail.artist.song_count.or_else(|| {
        let total: u32 = detail.albums.iter().map(|album| album.song_count).sum();
        (total > 0).then_some(total)
    })
}

pub fn info_line(detail: &ArtistDetail) -> String {
    [
        count_label(album_count(detail), "album", "albums"),
        count_label(song_count(detail), "song", "songs"),
    ]
    .into_iter()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join(" • ")
}

pub fn album_card(album: &Album) -> ArtistAlbumCard {
    ArtistAlbumCard {
        id: album.id.as_str().into(),
        name: album.name.as_str().into(),
        year: album
            .year
            .map(|year| year.to_string())
            .unwrap_or_default()
            .into(),
        image: Image::default(),
        has_image: false,
    }
}
