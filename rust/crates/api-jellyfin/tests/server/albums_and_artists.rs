use feishin_api::MusicServer;
use feishin_core::model::{AlbumListQuery, AlbumSort, ArtistListQuery, ArtistSort, SortOrder};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{USER, album_json, artist_json, items_path, list, page, server, song_json};

async fn mount_album_page_and_count(mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path(items_path()))
        .and(query_param("IncludeItemTypes", "MusicAlbum"))
        .and(query_param("SortBy", "DateCreated,SortName"))
        .and(query_param("SortOrder", "Descending"))
        .and(query_param("StartIndex", "20"))
        .and(query_param("Limit", "10"))
        .and(query_param("ArtistIds", "ar1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(vec![album_json("al1")], 321)))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path(items_path()))
        .and(query_param("IncludeItemTypes", "MusicAlbum"))
        .and(query_param("Limit", "1"))
        .and(query_param("StartIndex", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(vec![album_json("al1")], 321)))
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn album_list_and_count() {
    let mock = MockServer::start().await;
    mount_album_page_and_count(&mock).await;

    let jellyfin = server(&mock);
    let query = AlbumListQuery {
        sort: AlbumSort::RecentlyAdded,
        order: SortOrder::Desc,
        artist_ids: vec!["ar1".into()],
        ..AlbumListQuery::default()
    };
    let result = jellyfin.album_list(&query, page(20, 10)).await.unwrap();
    assert_eq!(result.total_count, Some(321));
    assert_eq!(result.start_index, 20);
    let album = &result.items[0];
    assert_eq!(album.name, "Album al1");
    assert_eq!(album.duration_ms, 3_600_000);
    assert_eq!(album.song_count, 12);
    assert_eq!(album.year, Some(1999));
    assert_eq!(album.cover_art_id.as_deref(), Some("al1"));
    assert_eq!(album.artists[0].id, "ar1");
    assert_eq!(album.play_count, 3);
    assert!(!album.favorite);

    let count_query = AlbumListQuery::default();
    assert_eq!(jellyfin.album_count(&count_query).await.unwrap(), 321);
}

#[tokio::test]
async fn album_detail_fetches_album_and_songs() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/users/{USER}/items/al1")))
        .and(query_param("Fields", "ChildCount"))
        .respond_with(ResponseTemplate::new(200).set_body_json(album_json("al1")))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path(items_path()))
        .and(query_param("ParentId", "al1"))
        .and(query_param(
            "SortBy",
            "ParentIndexNumber,IndexNumber,SortName",
        ))
        .and(query_param("UserId", USER))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(
            vec![song_json("s1", "al1"), song_json("s2", "al1")],
            2,
        )))
        .expect(1)
        .mount(&mock)
        .await;

    let detail = server(&mock).album("al1").await.unwrap();
    assert_eq!(detail.album.id, "al1");
    assert_eq!(detail.songs.len(), 2);
    assert_eq!(detail.songs[1].id, "s2");
}

async fn mount_artist_item_and_albums(mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path(format!("/users/{USER}/items/ar1")))
        .and(query_param("Fields", "Overview"))
        .respond_with(ResponseTemplate::new(200).set_body_json(artist_json("ar1")))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path(items_path()))
        .and(query_param("IncludeItemTypes", "MusicAlbum"))
        .and(query_param("ArtistIds", "ar1"))
        .and(query_param(
            "SortBy",
            "ProductionYear,PremiereDate,SortName",
        ))
        .and(query_param("SortOrder", "Descending"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(list(vec![album_json("al1"), album_json("al2")], 2)),
        )
        .expect(1)
        .mount(mock)
        .await;
}

async fn mount_artist_top_songs(mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path(items_path()))
        .and(query_param("IncludeItemTypes", "Audio"))
        .and(query_param("ArtistIds", "ar1"))
        .and(query_param("SortBy", "CommunityRating,SortName"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(list(vec![song_json("s1", "al1")], 1)),
        )
        .expect(1)
        .mount(mock)
        .await;
}

async fn mount_random_favorite_artists(mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/artists/albumArtists"))
        .and(query_param("SortBy", "Random,SortName"))
        .and(query_param("StartIndex", "5"))
        .and(query_param("Limit", "2"))
        .and(query_param("UserId", USER))
        .and(query_param("IsFavorite", "true"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(list(vec![artist_json("ar1"), artist_json("ar2")], 77)),
        )
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn artist_detail_and_list() {
    let mock = MockServer::start().await;
    mount_artist_item_and_albums(&mock).await;
    mount_artist_top_songs(&mock).await;
    mount_random_favorite_artists(&mock).await;

    let jellyfin = server(&mock);
    let detail = jellyfin.artist("ar1").await.unwrap();
    assert_eq!(detail.artist.name, "Artist ar1");
    assert_eq!(detail.artist.album_count, Some(4));
    assert_eq!(detail.artist.song_count, Some(40));
    assert_eq!(detail.artist.biography.as_deref(), Some("Bio"));
    assert_eq!(detail.artist.cover_art_id.as_deref(), Some("ar1"));
    assert!(detail.artist.favorite);
    assert_eq!(detail.albums.len(), 2);
    assert_eq!(detail.top_songs.len(), 1);

    let query = ArtistListQuery {
        sort: ArtistSort::Random,
        favorite: Some(true),
        ..ArtistListQuery::default()
    };
    let artists = jellyfin.artist_list(&query, page(5, 2)).await.unwrap();
    assert_eq!(artists.total_count, Some(77));
    assert_eq!(artists.items[1].id, "ar2");
}
