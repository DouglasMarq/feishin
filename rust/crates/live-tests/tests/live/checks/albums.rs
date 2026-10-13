use feishin_api::MusicServer;
use feishin_core::model::{AlbumListQuery, AlbumSort, Paging, SortOrder};

use crate::support::{
    ALBUM_COUNT, ALBUM_NAMES, Backend, COVER_ALBUM, album_named, album_names, artist_named,
    everything, genre_id_named, titles,
};

fn sorted(sort: AlbumSort, order: SortOrder) -> AlbumListQuery {
    AlbumListQuery {
        sort,
        order,
        ..AlbumListQuery::default()
    }
}

async fn listed_names(server: &dyn MusicServer, query: &AlbumListQuery) -> Vec<String> {
    let page = server
        .album_list(query, everything())
        .await
        .expect("albums");
    album_names(&page.items)
        .into_iter()
        .map(str::to_owned)
        .collect()
}

pub async fn pages_cover_all_albums(backend: Backend) {
    let server = backend.connect().await;
    let query = AlbumListQuery::default();
    let mut collected = Vec::new();
    for start_index in (0..ALBUM_COUNT).step_by(2) {
        let page = server
            .album_list(
                &query,
                Paging {
                    start_index,
                    limit: 2,
                },
            )
            .await
            .expect("album page");
        assert_eq!(page.start_index, start_index);
        assert_eq!(page.items.len(), 2);
        collected.extend(page.items.into_iter().map(|album| album.name));
    }
    assert_eq!(collected, ALBUM_NAMES);
    assert_eq!(
        server.album_count(&query).await.expect("count"),
        ALBUM_COUNT
    );
}

pub async fn sorts_by_name_and_year(backend: Backend) {
    let server = backend.connect().await;
    let ascending = listed_names(&*server, &sorted(AlbumSort::Name, SortOrder::Asc)).await;
    assert_eq!(ascending, ALBUM_NAMES);
    if backend.sorts_any_list() {
        let descending = listed_names(&*server, &sorted(AlbumSort::Name, SortOrder::Desc)).await;
        let mut reversed = ALBUM_NAMES.to_vec();
        reversed.reverse();
        assert_eq!(descending, reversed);
    }
    let mut by_year = listed_names(&*server, &sorted(AlbumSort::Year, SortOrder::Asc)).await;
    if backend.lists_years_newest_first() {
        by_year.reverse();
    }
    assert_eq!(
        by_year,
        ["First Light", "Second Wave", "Afterglow", "Third Signal"]
    );
}

pub async fn filters_by_artist_and_genre(backend: Backend) {
    let server = backend.connect().await;
    let artist = artist_named(&*server, "Aurora Tones").await;
    let by_artist = AlbumListQuery {
        artist_ids: vec![artist.id],
        ..AlbumListQuery::default()
    };
    let mut found = listed_names(&*server, &by_artist).await;
    found.sort();
    assert_eq!(found, ["Afterglow", "First Light"]);
    let by_genre = AlbumListQuery {
        genre_ids: vec![genre_id_named(&*server, "Rock").await],
        ..AlbumListQuery::default()
    };
    assert_eq!(listed_names(&*server, &by_genre).await, ["Second Wave"]);
    assert_eq!(server.album_count(&by_genre).await.expect("count"), 1);
}

pub async fn album_detail_lists_its_songs(backend: Backend) {
    let server = backend.connect().await;
    let listed = album_named(&*server, COVER_ALBUM).await;
    let detail = server.album(&listed.id).await.expect("album detail");
    assert_eq!(detail.album.name, COVER_ALBUM);
    assert_eq!(detail.album.year, Some(2019));
    assert_eq!(detail.album.song_count, 3);
    assert_eq!(detail.album.genres, ["Electronic"]);
    assert!(
        detail
            .album
            .album_artists
            .iter()
            .any(|artist| artist.name == "Aurora Tones")
    );
    assert!(detail.album.cover_art_id.is_some());
    assert_eq!(
        titles(&detail.songs),
        ["Dawn Chorus", "Morning Haze", "Noon Glare"]
    );
}
