use std::borrow::Borrow;

use feishin_core::model::{Album, AlbumSort, SortOrder};

use super::key::{SortValue, number, order_by, text};
use super::shuffle;

fn album_name(album: &Album) -> SortValue {
    text(Some(&album.name))
}

fn first_album_artist(album: &Album) -> SortValue {
    text(
        album
            .album_artists
            .first()
            .map(|artist| artist.name.as_str()),
    )
}

fn album_sort_value(album: &Album, sort: AlbumSort) -> SortValue {
    match sort {
        AlbumSort::Name | AlbumSort::Random => album_name(album),
        AlbumSort::AlbumArtist => first_album_artist(album),
        AlbumSort::Year => number(album.year.map(i64::from)),
        AlbumSort::PlayCount => number(Some(i64::from(album.play_count))),
        AlbumSort::Rating => number(album.user_rating.map(i64::from)),
        AlbumSort::Favorited => number(Some(i64::from(album.favorite))),
        AlbumSort::RecentlyAdded => SortValue::Text(album.created_at.clone()),
        AlbumSort::RecentlyPlayed => SortValue::Text(album.last_played_at.clone()),
        AlbumSort::SongCount => number(Some(i64::from(album.song_count))),
        AlbumSort::Duration => number(i64::try_from(album.duration_ms).ok()),
    }
}

fn album_of<Entry: Borrow<Album>>(entry: &Entry) -> &Album {
    <Entry as Borrow<Album>>::borrow(entry)
}

pub fn sort_albums<Entry: Borrow<Album>>(items: &mut [Entry], sort: AlbumSort, order: SortOrder) {
    let descending = order == SortOrder::Desc;
    match sort {
        AlbumSort::Random => shuffle(items),
        AlbumSort::AlbumArtist => {
            order_by(items, false, |entry| vec![album_name(album_of(entry))]);
            order_by(items, descending, |entry| {
                vec![first_album_artist(album_of(entry))]
            });
        }
        ordered => order_by(items, descending, |entry| {
            vec![album_sort_value(album_of(entry), ordered)]
        }),
    }
}
