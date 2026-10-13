use feishin_core::model::{Album, AlbumListQuery, AlbumSort, SortOrder};

use super::FakeServer;

const PARTIAL_LAST_ALBUM_SONGS: usize = 1_210;

fn materialized_key(album: &Album, sort: AlbumSort) -> (String, i64) {
    let numeric = |value: i64| (String::new(), value);
    let artist = album
        .album_artists
        .first()
        .map(|artist| artist.name.clone());
    match sort {
        AlbumSort::Name => (album.name.to_lowercase(), 0),
        AlbumSort::AlbumArtist => (artist.unwrap_or_default(), 0),
        AlbumSort::Year => numeric(i64::from(album.year.unwrap_or_default())),
        AlbumSort::SongCount => numeric(i64::from(album.song_count)),
        AlbumSort::Duration => numeric(album.duration_ms as i64),
        AlbumSort::Favorited => numeric(i64::from(album.favorite)),
        _ => numeric(0),
    }
}

fn materialized_order(server: &FakeServer, query: &AlbumListQuery) -> Vec<usize> {
    let search = query.search.as_deref().map(str::to_lowercase);
    let mut order: Vec<(usize, Album)> = (0..server.total_album_count())
        .map(|album| (album, server.album_at(album)))
        .filter(|(_, album)| {
            query
                .favorite
                .is_none_or(|favorite| favorite == album.favorite)
        })
        .filter(|(_, album)| {
            search.as_deref().is_none_or(|search| {
                album.name.to_lowercase().contains(search)
                    || album
                        .album_artists
                        .iter()
                        .any(|artist| artist.name.to_lowercase().contains(search))
            })
        })
        .collect();
    order.sort_by_key(|(index, album)| (materialized_key(album, query.sort), *index));
    let mut order: Vec<usize> = order.into_iter().map(|(index, _)| index).collect();
    if query.order == SortOrder::Desc {
        order.reverse();
    }
    order
}

#[test]
fn cheap_album_keys_sort_like_the_materialized_albums() {
    let server = FakeServer::new(PARTIAL_LAST_ALBUM_SONGS);
    let sorts = [
        AlbumSort::Name,
        AlbumSort::AlbumArtist,
        AlbumSort::Year,
        AlbumSort::SongCount,
        AlbumSort::Duration,
        AlbumSort::Favorited,
    ];
    for sort in sorts {
        for order in [SortOrder::Asc, SortOrder::Desc] {
            let query = AlbumListQuery {
                sort,
                order,
                ..AlbumListQuery::default()
            };
            let expected = materialized_order(&server, &query);
            assert_eq!(server.sorted_albums(&query), expected, "{sort:?} {order:?}");
        }
    }
}

#[test]
fn cheap_album_filters_match_the_materialized_albums() {
    let server = FakeServer::new(PARTIAL_LAST_ALBUM_SONGS);
    for favorite in [None, Some(true), Some(false)] {
        for search in [None, Some("a"), Some("ada")] {
            let query = AlbumListQuery {
                favorite,
                search: search.map(str::to_owned),
                ..AlbumListQuery::default()
            };
            let expected = materialized_order(&server, &query);
            assert_eq!(server.sorted_albums(&query), expected, "{query:?}");
        }
    }
}
