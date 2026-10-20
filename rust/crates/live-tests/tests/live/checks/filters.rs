use feishin_api::{ApiError, MusicServer};
use feishin_core::model::{AlbumListQuery, ArtistListQuery, ItemKind, SongListQuery};

use crate::support::{
    ALBUM_COUNT, Backend, SONG_COUNT, album_named, album_names, artist_named, artist_names,
    everything, titles,
};

async fn first_folder_id(server: &dyn MusicServer) -> String {
    server
        .music_folders()
        .await
        .expect("folders")
        .into_iter()
        .next()
        .expect("a music folder")
        .id
}

async fn titles_in_years(
    server: &dyn MusicServer,
    year_from: Option<i32>,
    year_to: Option<i32>,
) -> feishin_api::Result<Vec<String>> {
    let query = SongListQuery {
        year_from,
        year_to,
        ..SongListQuery::default()
    };
    let page = server.song_list(&query, everything()).await?;
    let count = server.song_count(&query).await?;
    assert_eq!(count, page.items.len(), "{year_from:?}..{year_to:?}");
    let mut found: Vec<String> = titles(&page.items).into_iter().map(str::to_owned).collect();
    found.sort();
    Ok(found)
}

pub async fn songs_filter_by_years(backend: Backend) {
    let server = backend.connect().await;
    let range = titles_in_years(&*server, Some(2021), Some(2022)).await;
    if !backend.filters_songs_by_year() {
        assert!(matches!(range, Err(ApiError::Unsupported(_))));
        return;
    }
    assert_eq!(
        range.expect("range"),
        ["Dusk Signal", "Tidal Pull", "Undertow"]
    );
    let single = titles_in_years(&*server, Some(2021), Some(2021)).await;
    assert_eq!(single.expect("single year"), ["Tidal Pull", "Undertow"]);
    let from_only = titles_in_years(&*server, Some(2022), None).await;
    assert_eq!(
        from_only.expect("from"),
        ["Blue Note", "Dusk Signal", "Late Set"]
    );
    let to_only = titles_in_years(&*server, None, Some(2019)).await;
    assert_eq!(
        to_only.expect("to"),
        ["Dawn Chorus", "Morning Haze", "Noon Glare"]
    );
    wide_year_ranges_cover_the_library(&*server).await;
}

async fn wide_year_ranges_cover_the_library(server: &dyn MusicServer) {
    let every_year = titles_in_years(server, Some(1), Some(9999)).await;
    assert_eq!(every_year.expect("every year").len(), SONG_COUNT);
    let up_to_2020 = titles_in_years(server, Some(1900), Some(2020)).await;
    assert_eq!(
        up_to_2020.expect("1900 to 2020"),
        ["Dawn Chorus", "Morning Haze", "Noon Glare"]
    );
}

pub async fn songs_filter_by_folder(backend: Backend) {
    let server = backend.connect().await;
    let in_folder = SongListQuery {
        music_folder_id: Some(first_folder_id(&*server).await),
        ..SongListQuery::default()
    };
    let folder_songs = server
        .song_list(&in_folder, everything())
        .await
        .expect("folder songs");
    assert_eq!(folder_songs.items.len(), SONG_COUNT);
    assert_eq!(
        server.song_count(&in_folder).await.expect("folder count"),
        SONG_COUNT
    );
}

pub async fn albums_filter_by_years_and_folder(backend: Backend) {
    let server = backend.connect().await;
    let by_years = AlbumListQuery {
        year_from: Some(2020),
        year_to: Some(2022),
        ..AlbumListQuery::default()
    };
    let page = server
        .album_list(&by_years, everything())
        .await
        .expect("albums by year");
    let mut found = album_names(&page.items);
    found.sort_unstable();
    assert_eq!(found, ["Afterglow", "Second Wave"]);
    assert_eq!(server.album_count(&by_years).await.expect("year count"), 2);
    let in_folder = AlbumListQuery {
        music_folder_id: Some(first_folder_id(&*server).await),
        ..AlbumListQuery::default()
    };
    assert_eq!(
        server.album_count(&in_folder).await.expect("folder count"),
        ALBUM_COUNT
    );
}

pub async fn counts_follow_filters(backend: Backend) {
    let server = backend.connect().await;
    let artist = artist_named(&*server, "Aurora Tones").await;
    let by_artist = SongListQuery {
        artist_ids: vec![artist.id.clone()],
        ..SongListQuery::default()
    };
    assert_eq!(
        server.song_count(&by_artist).await.expect("artist songs"),
        4
    );
    let album = album_named(&*server, "First Light").await;
    let by_album = SongListQuery {
        album_ids: vec![album.id],
        ..SongListQuery::default()
    };
    assert_eq!(server.song_count(&by_album).await.expect("album songs"), 3);
    let by_text = SongListQuery {
        search: Some("Morning".into()),
        ..SongListQuery::default()
    };
    assert_eq!(
        server.song_count(&by_text).await.expect("searched songs"),
        1
    );
    let albums_by_artist = AlbumListQuery {
        artist_ids: vec![artist.id],
        ..AlbumListQuery::default()
    };
    assert_eq!(
        server
            .album_count(&albums_by_artist)
            .await
            .expect("artist albums"),
        2
    );
}

struct FavoriteListings {
    albums: Vec<String>,
    album_count: usize,
    artists: Vec<String>,
}

async fn favorite_listings(server: &dyn MusicServer) -> FavoriteListings {
    let favorite_albums = AlbumListQuery {
        is_favorite: Some(true),
        ..AlbumListQuery::default()
    };
    let favorite_artists = ArtistListQuery {
        is_favorite: Some(true),
        ..ArtistListQuery::default()
    };
    let albums = server
        .album_list(&favorite_albums, everything())
        .await
        .expect("albums");
    let artists = server
        .artist_list(&favorite_artists, everything())
        .await
        .expect("artists");
    FavoriteListings {
        albums: album_names(&albums.items)
            .into_iter()
            .map(str::to_owned)
            .collect(),
        album_count: server.album_count(&favorite_albums).await.expect("count"),
        artists: artist_names(&artists.items)
            .into_iter()
            .map(str::to_owned)
            .collect(),
    }
}

async fn mark_favorites(
    server: &dyn MusicServer,
    album_id: &str,
    artist_id: &str,
    is_favorite: bool,
) {
    server
        .set_favorite(ItemKind::Album, &[album_id.to_owned()], is_favorite)
        .await
        .expect("album favorite");
    server
        .set_favorite(ItemKind::Artist, &[artist_id.to_owned()], is_favorite)
        .await
        .expect("artist favorite");
}

pub async fn favorite_filters_list_only_favorites(backend: Backend) {
    let server = backend.connect().await;
    let album = album_named(&*server, "Third Signal").await;
    let artist = artist_named(&*server, "Bravo Sine").await;
    mark_favorites(&*server, &album.id, &artist.id, true).await;
    let listings = favorite_listings(&*server).await;
    mark_favorites(&*server, &album.id, &artist.id, false).await;
    assert_eq!(listings.albums, ["Third Signal"]);
    assert_eq!(listings.album_count, 1);
    assert_eq!(listings.artists, ["Bravo Sine"]);
}
