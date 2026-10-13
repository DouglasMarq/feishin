use feishin_api::MusicServer;
use feishin_api_subsonic::SubsonicServer;
use feishin_core::model::{ItemKind, SongListQuery, SongSort};
use serde_json::{Value, json};
use wiremock::matchers::{path, query_param};
use wiremock::{Mock, MockServer};

use crate::support::{album, ok, paging, server, song};

const ALBUM_IDS: [&str; 3] = ["al-1", "al-2", "al-3"];
const SONGS_PER_ALBUM: usize = 4;
const PAGE_SIZE: usize = 5;

fn album_with_songs(album_id: &str) -> Value {
    let mut detail = album(album_id, album_id);
    let songs: Vec<Value> = (0..SONGS_PER_ALBUM)
        .map(|number| song(&format!("{album_id}-s{number}"), "Track"))
        .collect();
    detail["song"] = json!(songs);
    json!({ "album": detail })
}

async fn mount_discography(mock: &MockServer, expected_fetches: u64) {
    let albums: Vec<Value> = ALBUM_IDS.iter().map(|id| album(id, id)).collect();
    Mock::given(path("/rest/getArtist.view"))
        .and(query_param("id", "ar-1"))
        .respond_with(ok(json!({
            "artist": { "id": "ar-1", "name": "Artist", "album": albums }
        })))
        .expect(expected_fetches)
        .mount(mock)
        .await;
    for album_id in ALBUM_IDS {
        Mock::given(path("/rest/getAlbum.view"))
            .and(query_param("id", album_id))
            .respond_with(ok(album_with_songs(album_id)))
            .expect(expected_fetches)
            .mount(mock)
            .await;
    }
}

fn artist_songs() -> SongListQuery {
    SongListQuery {
        sort: SongSort::Album,
        artist_ids: vec!["ar-1".into()],
        ..SongListQuery::default()
    }
}

async fn read_every_artist_page(server: &SubsonicServer) -> Vec<String> {
    let query = artist_songs();
    let total = server.song_count(&query).await.unwrap_or(0);
    let mut song_ids = Vec::new();
    for start in (0..total).step_by(PAGE_SIZE) {
        let page = server.song_list(&query, paging(start, PAGE_SIZE)).await;
        song_ids.extend(
            page.into_iter()
                .flat_map(|page| page.items)
                .map(|song| song.id),
        );
    }
    song_ids
}

#[tokio::test]
async fn paging_an_artist_fetches_its_albums_once() {
    let mock = MockServer::start().await;
    mount_discography(&mock, 1).await;
    let server = server(&mock);

    let song_ids = read_every_artist_page(&server).await;
    assert_eq!(song_ids.len(), ALBUM_IDS.len() * SONGS_PER_ALBUM);
    assert_eq!(song_ids.first().map(String::as_str), Some("al-1-s0"));
    assert_eq!(song_ids.last().map(String::as_str), Some("al-3-s3"));
}

#[tokio::test]
async fn favoriting_a_song_refreshes_the_artist_songs() {
    let mock = MockServer::start().await;
    mount_discography(&mock, 2).await;
    Mock::given(path("/rest/star.view"))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    let server = server(&mock);

    read_every_artist_page(&server).await;
    assert!(
        server
            .set_favorite(ItemKind::Song, &["al-1-s0".into()], true)
            .await
            .is_ok()
    );
    assert_eq!(
        read_every_artist_page(&server).await.len(),
        ALBUM_IDS.len() * SONGS_PER_ALBUM
    );
}
