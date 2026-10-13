use feishin_api::MusicServer;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{
    AudioFormat, mount_ping, native_album_json, native_song_json, server, subsonic_entry,
    subsonic_ok,
};

#[tokio::test]
async fn album_detail_fetches_album_and_tracks() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.58.0", &[]).await;
    Mock::given(method("GET"))
        .and(path("/api/album/al1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(native_album_json("al1")))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/song"))
        .and(query_param("album_id", "al1"))
        .and(query_param("_start", "0"))
        .and(query_param("_end", "0"))
        .and(query_param("_sort", "album"))
        .and(query_param("_order", "ASC"))
        .and(query_param("missing", "false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            native_song_json("s1", "One"),
            native_song_json("s2", "Two")
        ])))
        .expect(1)
        .mount(&mock)
        .await;

    let detail = server(&mock, Some("jwt-1"))
        .album("al1")
        .await
        .expect("album");
    assert_eq!(detail.album.name, "Album One");
    assert_eq!(detail.album.year, Some(2001));
    assert_eq!(detail.album.duration_ms, 2_400_500);
    assert_eq!(detail.album.album_artists[0].id, "ar1");
    assert_eq!(detail.album.cover_art_id.as_deref(), Some("al1"));
    assert_eq!(detail.songs.len(), 2);
}

async fn mount_native_artist(mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/artist/ar1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "ar1",
            "name": "Artist One",
            "albumCount": 1,
            "songCount": 10,
            "size": 1,
            "biography": "Bio",
            "starred": true,
            "rating": 5,
            "stats": {
                "albumartist": { "albumCount": 2, "songCount": 12, "size": 1 }
            }
        })))
        .expect(1)
        .mount(mock)
        .await;
}

async fn mount_artist_albums(mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/album"))
        .and(query_param("artist_id", "ar1"))
        .and(query_param("_sort", "max_year"))
        .and(query_param("_order", "DESC"))
        .and(query_param("_start", "0"))
        .and(query_param("_end", "-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-total-count", "2")
                .set_body_json(json!([native_album_json("al1"), native_album_json("al2")])),
        )
        .expect(1)
        .mount(mock)
        .await;
}

async fn mount_top_songs(mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/getTopSongs.view"))
        .and(query_param("id", "ar1"))
        .respond_with(subsonic_ok(json!({
            "topSongs": { "song": [subsonic_entry("s9", "Hit", "al1", AudioFormat::Flac)] }
        })))
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn artist_detail_combines_artist_albums_and_top_songs() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.58.0", &["topSongsByArtistId"]).await;
    mount_native_artist(&mock).await;
    mount_artist_albums(&mock).await;
    mount_top_songs(&mock).await;

    let detail = server(&mock, Some("jwt-1"))
        .artist("ar1")
        .await
        .expect("artist");
    assert_eq!(detail.artist.name, "Artist One");
    assert_eq!(detail.artist.album_count, Some(2));
    assert_eq!(detail.artist.song_count, Some(12));
    assert_eq!(detail.artist.biography.as_deref(), Some("Bio"));
    assert_eq!(detail.artist.user_rating, Some(5));
    assert_eq!(detail.albums.len(), 2);
    assert_eq!(detail.top_songs.len(), 1);
    assert_eq!(detail.top_songs[0].title, "Hit");
}
