use feishin_core::model::{ArtistSort, SortOrder};

use super::key::{SortValue, number, order_by, text};
use crate::types::SubsonicArtist;

fn artist_name(artist: &SubsonicArtist) -> SortValue {
    text(Some(&artist.name))
}

pub fn sort_artists(items: &mut [SubsonicArtist], sort: ArtistSort, order: SortOrder) {
    let descending = order == SortOrder::Desc;
    match sort {
        ArtistSort::AlbumCount => {
            order_by(items, false, |artist| vec![artist_name(artist)]);
            order_by(items, descending, |artist| {
                vec![number(artist.album_count.map(|count| count as i64))]
            });
        }
        ArtistSort::Favorited => order_by(items, descending, |artist| {
            vec![SortValue::Text(artist.starred.sort_value())]
        }),
        ArtistSort::Name => order_by(items, descending, |artist| vec![artist_name(artist)]),
        ArtistSort::Rating => order_by(items, descending, |artist| {
            vec![number(artist.user_rating.map(|rating| rating as i64))]
        }),
        ArtistSort::SongCount | ArtistSort::PlayCount | ArtistSort::Random => {}
    }
}
