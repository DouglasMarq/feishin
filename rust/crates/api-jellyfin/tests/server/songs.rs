use feishin_api::MusicServer;
use feishin_core::model::{Song, SongListQuery, SongSort, SortOrder};
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{authed, items_path, list, page, sample_song, server, song_json};

async fn mount_filtered_song_list(mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path(items_path()))
        .and(authed())
        .and(query_param("IncludeItemTypes", "Audio"))
        .and(query_param("StartIndex", "100"))
        .and(query_param("Limit", "50"))
        .and(query_param("SortBy", "PlayCount,SortName"))
        .and(query_param("SortOrder", "Descending"))
        .and(query_param("Recursive", "true"))
        .and(query_param("IsFavorite", "true"))
        .and(query_param("GenreIds", "g1,g2"))
        .and(query_param("Years", "2000,2001,2002"))
        .and(query_param("ParentId", "lib1"))
        .and(query_param("SearchTerm", "foo"))
        .and(query_param("Fields", "MediaSources"))
        .and(query_param("Fields", "Genres"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(
            vec![song_json("s1", "al1"), song_json("s2", "al1")],
            1234,
        )))
        .expect(1)
        .mount(mock)
        .await;
}

fn filtered_song_query() -> SongListQuery {
    SongListQuery {
        sort: SongSort::PlayCount,
        order: SortOrder::Desc,
        search: Some("foo".into()),
        favorite: Some(true),
        genre_ids: vec!["g1".into(), "g2".into()],
        year_from: Some(2000),
        year_to: Some(2002),
        music_folder_id: Some("lib1".into()),
        ..SongListQuery::default()
    }
}

fn assert_song_is_normalized(song: &Song) {
    assert_eq!(song.id, "s1");
    assert_eq!(song.server_id, "srv1");
    assert_eq!(song.title, "Song s1");
    assert_eq!(song.duration_ms, 245_500);
    assert_eq!(song.track_number, Some(3));
    assert_eq!(song.disc_number, Some(2));
    assert_eq!(song.year, Some(2010));
    assert_eq!(song.bitrate_kbps, Some(912));
    assert_eq!(song.play_count, 7);
    assert!(song.favorite);
    assert_eq!(song.user_rating, None);
    assert_eq!(song.artists[0].name, "Artist B");
    assert_eq!(song.album_artists[0].id, "ar1");
    assert_eq!(song.album_id.as_deref(), Some("al1"));
    assert_eq!(song.cover_art_id.as_deref(), Some("al1"));
    assert_eq!(song.genres, vec!["Rock".to_string()]);
    assert_eq!(song.container.as_deref(), Some("flac"));
    assert_eq!(song.size_bytes, Some(31_000_000));
    assert_eq!(song.path.as_deref(), Some("/music/a/03.flac"));
    assert_eq!(song.replay_gain.unwrap().track_gain_db, Some(-6.5));
    assert_eq!(
        song.last_played_at.as_deref(),
        Some("2024-02-03T04:05:06.0000000Z")
    );
}

#[tokio::test]
async fn song_list_pages_sorts_filters_and_normalizes() {
    let mock = MockServer::start().await;
    mount_filtered_song_list(&mock).await;

    let result = server(&mock)
        .song_list(&filtered_song_query(), page(100, 50))
        .await
        .unwrap();

    assert_eq!(result.start_index, 100);
    assert_eq!(result.total_count, Some(1234));
    assert_eq!(result.items.len(), 2);
    assert_song_is_normalized(&result.items[0]);
}

#[tokio::test]
async fn song_count_uses_total_record_count() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(items_path()))
        .and(query_param("Limit", "1"))
        .and(query_param("StartIndex", "0"))
        .and(query_param("SortBy", "Album,SortName"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(list(vec![song_json("s1", "a")], 987)),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let count = server(&mock)
        .song_count(&SongListQuery {
            sort: SongSort::Rating,
            ..SongListQuery::default()
        })
        .await
        .unwrap();
    assert_eq!(count, 987);
}

#[tokio::test]
async fn song_list_by_album_drops_name_matches_from_other_albums() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(items_path()))
        .and(query_param("AlbumIds", "al1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(
            vec![song_json("s1", "al1"), song_json("s2", "other")],
            2,
        )))
        .mount(&mock)
        .await;

    let query = SongListQuery {
        album_ids: vec!["al1".into()],
        ..SongListQuery::default()
    };
    let result = server(&mock).song_list(&query, page(0, 100)).await.unwrap();
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.total_count, Some(1));
    assert_eq!(server(&mock).song_count(&query).await.unwrap(), 1);
}

#[tokio::test]
async fn song_list_batches_many_album_ids() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(items_path()))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(list(vec![song_json("s1", "a0")], 1)),
        )
        .expect(2)
        .mount(&mock)
        .await;

    let query = SongListQuery {
        album_ids: (0..60).map(|index| format!("a{index}")).collect(),
        ..SongListQuery::default()
    };
    let result = server(&mock).song_list(&query, page(0, 500)).await.unwrap();
    assert_eq!(result.items.len(), 2);
    assert_eq!(result.total_count, Some(2));
}

async fn mount_random_and_similar_mixes(mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path(items_path()))
        .and(query_param("SortBy", "Random,SortName"))
        .and(query_param("Limit", "5"))
        .and(query_param("GenreIds", "g1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(vec![song_json("s1", "a")], 1)))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/items/s1/similar"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(vec![song_json("s1", "a")], 1)))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/items/s1/InstantMix"))
        .and(query_param("Limit", "3"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(list(vec![song_json("s1", "a"), song_json("s9", "a")], 2)),
        )
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn random_and_similar_songs() {
    let mock = MockServer::start().await;
    mount_random_and_similar_mixes(&mock).await;

    let jellyfin = server(&mock);
    assert_eq!(jellyfin.random_songs(5, Some("g1")).await.unwrap().len(), 1);
    let similar = jellyfin.similar_songs("s1", 3).await.unwrap();
    assert_eq!(similar.len(), 1);
    assert_eq!(similar[0].id, "s9");
}

#[tokio::test]
async fn lyrics_synced_and_missing() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/audio/s1/Lyrics"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "Lyrics": [
                { "Start": 10_000_000, "Text": "Hello" },
                { "Start": 25_000_000, "Text": "World" }
            ]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/audio/s2/Lyrics"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock)
        .await;

    let jellyfin = server(&mock);
    let mut song = sample_song("s1");
    assert_eq!(
        jellyfin.lyrics(&song).await.unwrap().as_deref(),
        Some("[00:01.00]Hello\n[00:02.50]World")
    );
    song.id = "s2".into();
    assert_eq!(jellyfin.lyrics(&song).await.unwrap(), None);
}
