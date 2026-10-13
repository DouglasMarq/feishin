use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use feishin_api::MusicServer;
use feishin_api_subsonic::SubsonicServer;
use feishin_core::model::{
    AlbumListQuery, ArtistListQuery, ItemKind, Page, Playlist, SongListQuery, SongSort,
};
use serde_json::{Value, json};
use wiremock::matchers::{path, query_param};
use wiremock::{Mock, MockServer, Request};

use crate::support::{album, artist, offset_response, ok, paging, playlist, server, song};

const ARTIST_COUNT: usize = 250;
const ARTIST_PAGE_SIZE: usize = 50;

fn artist_index_body() -> Value {
    let artists: Vec<Value> = (0..ARTIST_COUNT)
        .map(|index| artist(&format!("ar-{index:03}"), &format!("Artist {index:03}"), 1))
        .collect();
    json!({ "artists": { "index": [{ "name": "A", "artist": artists }] } })
}

async fn mount_artist_index(mock: &MockServer, folder: &str, expected_fetches: u64) {
    Mock::given(path("/rest/getArtists.view"))
        .and(query_param("musicFolderId", folder))
        .respond_with(ok(artist_index_body()))
        .expect(expected_fetches)
        .mount(mock)
        .await;
}

fn folder_query(folder: &str) -> ArtistListQuery {
    ArtistListQuery {
        music_folder_id: Some(folder.into()),
        ..Default::default()
    }
}

async fn artist_page(server: &SubsonicServer, folder: &str, page_number: usize) -> Vec<String> {
    let start = page_number * ARTIST_PAGE_SIZE;
    server
        .artist_list(&folder_query(folder), paging(start, ARTIST_PAGE_SIZE))
        .await
        .map(|page| page.items.into_iter().map(|artist| artist.id).collect())
        .unwrap_or_default()
}

#[tokio::test]
async fn scrolling_five_artist_pages_fetches_the_index_once() {
    let mock = MockServer::start().await;
    mount_artist_index(&mock, "1", 1).await;
    let server = server(&mock);

    for page_number in 0..5 {
        let ids = artist_page(&server, "1", page_number).await;
        assert_eq!(ids.len(), ARTIST_PAGE_SIZE);
        assert_eq!(
            ids.first().map(String::as_str),
            Some(format!("ar-{:03}", page_number * ARTIST_PAGE_SIZE).as_str())
        );
    }
}

#[tokio::test]
async fn concurrent_artist_pages_share_one_index_fetch() {
    let mock = MockServer::start().await;
    mount_artist_index(&mock, "1", 1).await;
    let server = server(&mock);

    let (first, second, third) = tokio::join!(
        artist_page(&server, "1", 0),
        artist_page(&server, "1", 1),
        artist_page(&server, "1", 2),
    );
    assert_eq!(
        first.len() + second.len() + third.len(),
        3 * ARTIST_PAGE_SIZE
    );
}

#[tokio::test]
async fn artist_index_is_cached_per_music_folder() {
    let mock = MockServer::start().await;
    mount_artist_index(&mock, "1", 1).await;
    mount_artist_index(&mock, "2", 1).await;
    let server = server(&mock);

    for folder in ["1", "2", "1", "2"] {
        assert_eq!(
            artist_page(&server, folder, 0).await.len(),
            ARTIST_PAGE_SIZE
        );
    }
}

async fn mount_annotation_writes(mock: &MockServer) {
    for endpoint in ["/rest/star.view", "/rest/setRating.view"] {
        Mock::given(path(endpoint))
            .respond_with(ok(json!({})))
            .expect(1)
            .mount(mock)
            .await;
    }
}

#[tokio::test]
async fn favorites_and_ratings_refresh_the_artist_index() {
    let mock = MockServer::start().await;
    mount_artist_index(&mock, "1", 3).await;
    mount_annotation_writes(&mock).await;
    let server = server(&mock);

    artist_page(&server, "1", 0).await;
    assert!(
        server
            .set_favorite(ItemKind::Artist, &["ar-001".into()], true)
            .await
            .is_ok()
    );
    artist_page(&server, "1", 1).await;
    artist_page(&server, "1", 2).await;
    assert!(
        server
            .set_rating(ItemKind::Artist, "ar-001", 4)
            .await
            .is_ok()
    );
    artist_page(&server, "1", 3).await;
}

fn starred_body() -> Value {
    let mut starred_song = song("s1", "One");
    starred_song["starred"] = json!("2024-06-01T00:00:00Z");
    let mut starred_album = album("a1", "Alpha");
    starred_album["starred"] = json!("2024-06-01T00:00:00Z");
    json!({ "starred2": { "song": [starred_song], "album": [starred_album, album("a2", "Beta")] } })
}

async fn mount_starred(mock: &MockServer, expected_fetches: u64) {
    Mock::given(path("/rest/getStarred2.view"))
        .respond_with(ok(starred_body()))
        .expect(expected_fetches)
        .mount(mock)
        .await;
}

async fn read_every_favorites_view(server: &SubsonicServer) -> (usize, usize) {
    let albums = AlbumListQuery {
        favorite: Some(true),
        ..Default::default()
    };
    let songs = SongListQuery {
        favorite: Some(true),
        sort: SongSort::Name,
        ..Default::default()
    };
    let (album_page, album_count, song_page, song_count) = tokio::join!(
        server.album_list(&albums, paging(0, 1)),
        server.album_count(&albums),
        server.song_list(&songs, paging(0, 10)),
        server.song_count(&songs),
    );
    assert_eq!(album_page.map(|page| page.items.len()).ok(), Some(1));
    assert_eq!(song_page.map(|page| page.items.len()).ok(), Some(1));
    (album_count.unwrap_or(0), song_count.unwrap_or(0))
}

#[tokio::test]
async fn favorite_lists_and_counts_share_one_starred_fetch() {
    let mock = MockServer::start().await;
    mount_starred(&mock, 1).await;
    let server = server(&mock);

    assert_eq!(read_every_favorites_view(&server).await, (2, 1));
    assert_eq!(read_every_favorites_view(&server).await, (2, 1));
}

#[tokio::test]
async fn unstarring_refreshes_the_starred_lists() {
    let mock = MockServer::start().await;
    mount_starred(&mock, 2).await;
    Mock::given(path("/rest/unstar.view"))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    let server = server(&mock);

    read_every_favorites_view(&server).await;
    assert!(
        server
            .set_favorite(ItemKind::Song, &["s1".into()], false)
            .await
            .is_ok()
    );
    read_every_favorites_view(&server).await;
}

async fn mount_playlists(mock: &MockServer, expected_fetches: u64) {
    Mock::given(path("/rest/getPlaylists.view"))
        .respond_with(ok(json!({
            "playlists": { "playlist": [playlist("p1", "One"), playlist("p2", "Two")] }
        })))
        .expect(expected_fetches)
        .mount(mock)
        .await;
    Mock::given(path("/rest/updatePlaylist.view"))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(mock)
        .await;
}

fn playlist_names(page: Option<Page<Playlist>>) -> Vec<String> {
    page.map(|page| {
        page.items
            .into_iter()
            .map(|playlist| playlist.name)
            .collect()
    })
    .unwrap_or_default()
}

#[tokio::test]
async fn playlist_pages_are_cached_until_a_playlist_changes() {
    let mock = MockServer::start().await;
    mount_playlists(&mock, 2).await;
    let server = server(&mock);

    assert_eq!(
        playlist_names(server.playlist_list(paging(0, 1)).await.ok()),
        ["One"]
    );
    assert_eq!(
        playlist_names(server.playlist_list(paging(1, 1)).await.ok()),
        ["Two"]
    );
    assert!(server.add_to_playlist("p1", &["s1".into()]).await.is_ok());
    assert_eq!(
        playlist_names(server.playlist_list(paging(0, 2)).await.ok()).len(),
        2
    );
}

#[tokio::test]
async fn search_count_probes_instead_of_paging_every_match() {
    let mock = MockServer::start().await;
    let total = 20_000;
    let downloaded_songs = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&downloaded_songs);
    Mock::given(path("/rest/search3.view"))
        .and(query_param("query", "love"))
        .respond_with(move |request: &Request| {
            let songs = offset_response(request, total, "songOffset", "songCount");
            counter.fetch_add(songs.len(), Ordering::SeqCst);
            ok(json!({ "searchResult3": { "song": songs } }))
        })
        .mount(&mock)
        .await;
    let query = SongListQuery {
        search: Some("love".into()),
        ..Default::default()
    };

    assert_eq!(server(&mock).song_count(&query).await.ok(), Some(total));
    let requests = mock.received_requests().await.unwrap_or_default().len();
    assert!(requests <= 20, "{requests} requests");
    assert!(downloaded_songs.load(Ordering::SeqCst) <= 520);
}
