use std::borrow::Borrow;

use feishin_core::model::{ArtistSort, SortOrder};

use super::key::{SortValue, number, order_by, text};
use crate::types::SubsonicArtist;

fn artist_name(artist: &SubsonicArtist) -> SortValue {
    text(Some(&artist.name))
}

fn artist_of<Entry: Borrow<SubsonicArtist>>(entry: &Entry) -> &SubsonicArtist {
    <Entry as Borrow<SubsonicArtist>>::borrow(entry)
}

pub fn sort_artists<Entry: Borrow<SubsonicArtist>>(
    items: &mut [Entry],
    sort: ArtistSort,
    order: SortOrder,
) {
    let descending = order == SortOrder::Desc;
    match sort {
        ArtistSort::AlbumCount => {
            order_by(items, false, |entry| vec![artist_name(artist_of(entry))]);
            order_by(items, descending, |entry| {
                vec![number(
                    artist_of(entry).album_count.map(|count| count as i64),
                )]
            });
        }
        ArtistSort::Favorited => order_by(items, descending, |entry| {
            vec![SortValue::Text(artist_of(entry).starred.sort_value())]
        }),
        ArtistSort::Name => order_by(items, descending, |entry| {
            vec![artist_name(artist_of(entry))]
        }),
        ArtistSort::Rating => order_by(items, descending, |entry| {
            vec![number(
                artist_of(entry).user_rating.map(|rating| rating as i64),
            )]
        }),
        ArtistSort::SongCount | ArtistSort::PlayCount | ArtistSort::Random => {}
    }
}
