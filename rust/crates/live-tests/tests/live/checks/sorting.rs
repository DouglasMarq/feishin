use feishin_api::MusicServer;
use feishin_core::model::{
    AlbumListQuery, AlbumSort, ArtistListQuery, ArtistSort, SongListQuery, SongSort, SortOrder,
};

use crate::support::{
    ALBUM_NAMES, ARTIST_NAMES, Backend, SONG_TITLES, album_names, artist_names, everything, titles,
};

const ORDERS: [SortOrder; 2] = [SortOrder::Asc, SortOrder::Desc];
const SONG_SORTS: [SongSort; 12] = [
    SongSort::Name,
    SongSort::Album,
    SongSort::AlbumArtist,
    SongSort::Artist,
    SongSort::Duration,
    SongSort::Year,
    SongSort::PlayCount,
    SongSort::Rating,
    SongSort::Favorited,
    SongSort::RecentlyAdded,
    SongSort::RecentlyPlayed,
    SongSort::Random,
];
const ALBUM_SORTS: [AlbumSort; 11] = [
    AlbumSort::Name,
    AlbumSort::AlbumArtist,
    AlbumSort::Year,
    AlbumSort::PlayCount,
    AlbumSort::Rating,
    AlbumSort::Favorited,
    AlbumSort::RecentlyAdded,
    AlbumSort::RecentlyPlayed,
    AlbumSort::SongCount,
    AlbumSort::Duration,
    AlbumSort::Random,
];
const ARTIST_SORTS: [ArtistSort; 7] = [
    ArtistSort::Name,
    ArtistSort::AlbumCount,
    ArtistSort::SongCount,
    ArtistSort::PlayCount,
    ArtistSort::Rating,
    ArtistSort::Favorited,
    ArtistSort::Random,
];

fn sorted_copy(names: Vec<&str>) -> Vec<String> {
    let mut owned: Vec<String> = names.into_iter().map(str::to_owned).collect();
    owned.sort();
    owned
}

pub async fn every_song_sort_lists_the_library(backend: Backend) {
    let server = backend.connect().await;
    for sort in SONG_SORTS {
        for order in ORDERS {
            let query = SongListQuery {
                sort,
                order,
                ..SongListQuery::default()
            };
            let page = server
                .song_list(&query, everything())
                .await
                .unwrap_or_else(|error| panic!("{sort:?} {order:?}: {error}"));
            assert_eq!(
                sorted_copy(titles(&page.items)),
                SONG_TITLES,
                "{sort:?} {order:?}"
            );
        }
    }
}

pub async fn every_album_sort_lists_all_albums(backend: Backend) {
    let server = backend.connect().await;
    for sort in ALBUM_SORTS {
        for order in ORDERS {
            let query = AlbumListQuery {
                sort,
                order,
                ..AlbumListQuery::default()
            };
            let page = server
                .album_list(&query, everything())
                .await
                .unwrap_or_else(|error| panic!("{sort:?} {order:?}: {error}"));
            let names = sorted_copy(album_names(&page.items));
            let expected = match (backend.sorts_any_list(), sort) {
                (
                    false,
                    AlbumSort::Favorited | AlbumSort::RecentlyPlayed | AlbumSort::PlayCount,
                ) => {
                    continue;
                }
                _ => ALBUM_NAMES,
            };
            assert_eq!(names, expected, "{sort:?} {order:?}");
        }
    }
}

pub async fn every_artist_sort_lists_all_artists(backend: Backend) {
    let server = backend.connect().await;
    for sort in ARTIST_SORTS {
        for order in ORDERS {
            let query = ArtistListQuery {
                sort,
                order,
                ..ArtistListQuery::default()
            };
            let page = server
                .artist_list(&query, everything())
                .await
                .unwrap_or_else(|error| panic!("{sort:?} {order:?}: {error}"));
            assert_eq!(
                sorted_copy(artist_names(&page.items)),
                ARTIST_NAMES,
                "{sort:?} {order:?}"
            );
        }
    }
}

async fn artists_with_most_albums_come_first(server: &dyn MusicServer) {
    let by_album_count = ArtistListQuery {
        sort: ArtistSort::AlbumCount,
        order: SortOrder::Desc,
        ..ArtistListQuery::default()
    };
    let artists = server
        .artist_list(&by_album_count, everything())
        .await
        .expect("artists by album count");
    assert_eq!(artist_names(&artists.items).first(), Some(&"Aurora Tones"));
}

pub async fn count_sorts_order_by_the_counts(backend: Backend) {
    let server = backend.connect().await;
    if !backend.sorts_by_counts() {
        return;
    }
    artists_with_most_albums_come_first(&*server).await;
    let by_song_count = AlbumListQuery {
        sort: AlbumSort::SongCount,
        order: SortOrder::Desc,
        ..AlbumListQuery::default()
    };
    let albums = server
        .album_list(&by_song_count, everything())
        .await
        .expect("albums by song count");
    if backend.sorts_any_list() {
        assert_eq!(album_names(&albums.items).first(), Some(&"First Light"));
        assert_eq!(album_names(&albums.items).last(), Some(&"Afterglow"));
    }
}
