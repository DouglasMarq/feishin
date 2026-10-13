use feishin_api::{MusicServer, Result};
use feishin_core::model::{
    Album, AlbumListQuery, Page, Paging, SongListQuery, SongSort, SortOrder,
};

use super::FakeServer;

#[tokio::test]
async fn pages_cover_the_whole_sorted_list() {
    let server = FakeServer::new(1000);
    let query = SongListQuery {
        sort: SongSort::Name,
        ..SongListQuery::default()
    };
    assert_eq!(server.song_count(&query).await.ok(), Some(1000));
    let first = server
        .song_list(
            &query,
            Paging {
                start_index: 0,
                limit: 10,
            },
        )
        .await
        .map(|page| page.items)
        .unwrap_or_default();
    let second = server
        .song_list(
            &query,
            Paging {
                start_index: 10,
                limit: 10,
            },
        )
        .await
        .map(|page| page.items)
        .unwrap_or_default();
    assert!(
        first.last().map(|song| song.title.to_lowercase())
            <= second.first().map(|song| song.title.to_lowercase())
    );
}

#[tokio::test]
async fn albums_filter_and_sort() {
    let server = FakeServer::new(1200);
    let all = AlbumListQuery::default();
    assert_eq!(
        MusicServer::album_count(&server, &all).await.ok(),
        Some(100)
    );
    let favorites = AlbumListQuery {
        favorite: Some(true),
        ..AlbumListQuery::default()
    };
    let count = MusicServer::album_count(&server, &favorites)
        .await
        .unwrap_or_default();
    assert!(count > 0 && count < 100);
    let paging = Paging {
        start_index: 0,
        limit: 100,
    };
    let ids = |page: Result<Page<Album>>| -> Vec<String> {
        page.map(|page| page.items.into_iter().map(|album| album.id).collect())
            .unwrap_or_default()
    };
    let mut ascending = ids(server.album_list(&all, paging).await);
    let descending_query = AlbumListQuery {
        order: SortOrder::Desc,
        ..all
    };
    let descending = ids(server.album_list(&descending_query, paging).await);
    ascending.reverse();
    assert_eq!(ascending, descending);
}

#[tokio::test]
async fn search_filters_songs() {
    let server = FakeServer::new(1000);
    let query = SongListQuery {
        search: Some("river".into()),
        ..SongListQuery::default()
    };
    let count = server.song_count(&query).await.unwrap_or_default();
    assert!(count > 0 && count < 1000);
}

#[tokio::test]
async fn search_finds_albums_and_artists() {
    let server = FakeServer::new(1000);
    let results = server.search("ada", 5).await.ok();
    let (albums, artists) = results.map_or((0, 0), |results| {
        (results.albums.len(), results.artists.len())
    });
    assert!(albums > 0 && albums <= 5);
    assert!(artists > 0 && artists <= 5);
}
