use feishin_api::MusicServer;
use feishin_api_navidrome::NavidromeServer;
use feishin_core::model::{ArtistListQuery, ItemKind, Paging, SortOrder};
use serde_json::{Value, json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer};

use crate::support::{server, subsonic_ok};

const ARTIST_PAGE_SIZE: usize = 2;

fn artist_index() -> Value {
    let artists: Vec<Value> = (0..10)
        .map(|index| json!({ "id": format!("ar{index}"), "name": format!("Artist {index}") }))
        .collect();
    json!({ "artists": { "index": [{ "name": "A", "artist": artists }] } })
}

async fn mount_subsonic(mock: &MockServer, endpoint: &str, body: Value, expected_calls: u64) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/{endpoint}.view")))
        .respond_with(subsonic_ok(body))
        .expect(expected_calls)
        .mount(mock)
        .await;
}

async fn artist_page_ids(
    server: &NavidromeServer,
    page_number: usize,
    order: SortOrder,
) -> Vec<String> {
    let query = ArtistListQuery {
        order,
        ..ArtistListQuery::default()
    };
    let paging = Paging {
        start_index: page_number * ARTIST_PAGE_SIZE,
        limit: ARTIST_PAGE_SIZE,
    };
    server
        .artist_list(&query, paging)
        .await
        .map(|page| page.items.into_iter().map(|artist| artist.id).collect())
        .unwrap_or_default()
}

#[tokio::test]
async fn fallback_artist_pages_fetch_the_index_once() {
    let mock = MockServer::start().await;
    mount_subsonic(&mock, "getArtists", artist_index(), 1).await;
    let server = server(&mock, None);

    for page_number in 0..5 {
        let ids = artist_page_ids(&server, page_number, SortOrder::Asc).await;
        assert_eq!(ids.first(), Some(&format!("ar{}", page_number * 2)));
    }
    assert_eq!(
        artist_page_ids(&server, 0, SortOrder::Desc).await,
        ["ar9", "ar8"]
    );
}

#[tokio::test]
async fn fallback_artist_index_refreshes_after_a_favorite_change() {
    let mock = MockServer::start().await;
    mount_subsonic(&mock, "getArtists", artist_index(), 2).await;
    mount_subsonic(&mock, "star", json!({}), 1).await;
    let server = server(&mock, None);

    artist_page_ids(&server, 0, SortOrder::Asc).await;
    artist_page_ids(&server, 1, SortOrder::Asc).await;
    assert!(
        server
            .set_favorite(ItemKind::Artist, &["ar1".into()], true)
            .await
            .is_ok()
    );
    artist_page_ids(&server, 2, SortOrder::Asc).await;
}

#[tokio::test]
async fn fallback_playlist_pages_are_cached_until_a_playlist_is_deleted() {
    let mock = MockServer::start().await;
    let playlists = json!({ "playlists": { "playlist": [
        { "id": "p1", "name": "One" }, { "id": "p2", "name": "Two" }
    ] } });
    mount_subsonic(&mock, "getPlaylists", playlists, 2).await;
    mount_subsonic(&mock, "deletePlaylist", json!({}), 1).await;
    let server = server(&mock, None);
    let first = Paging {
        start_index: 0,
        limit: 1,
    };
    let second = Paging {
        start_index: 1,
        limit: 1,
    };

    assert!(server.playlist_list(first).await.is_ok());
    assert!(server.playlist_list(second).await.is_ok());
    assert!(server.delete_playlist("p2").await.is_ok());
    assert!(server.playlist_list(first).await.is_ok());
}
