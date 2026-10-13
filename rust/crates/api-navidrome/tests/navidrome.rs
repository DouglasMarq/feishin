use feishin_api::{ApiError, Credential, MusicServer, StreamOptions};
use feishin_api_navidrome::NavidromeServer;
use feishin_core::model::{
    ItemKind, Paging, ServerConfig, ServerType, SongListQuery, SongSort, SortOrder,
};
use serde_json::{Value, json};
use wiremock::matchers::{body_json, header, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const SUBSONIC_AUTH: &str = "u=alice&s=salt1&t=tok1";

fn config(url: &str) -> ServerConfig {
    ServerConfig {
        id: "srv1".into(),
        name: "Home".into(),
        server_type: ServerType::Navidrome,
        url: format!("{url}/"),
        username: "alice".into(),
    }
}

fn credential(token: Option<&str>) -> Credential {
    Credential::Navidrome {
        subsonic: SUBSONIC_AUTH.into(),
        nd_token: token.map(str::to_owned),
    }
}

fn server(mock: &MockServer, token: Option<&str>) -> NavidromeServer {
    NavidromeServer::with_credential(config(&mock.uri()), &credential(token)).expect("server")
}

fn subsonic_ok(extra: Value) -> ResponseTemplate {
    let mut body = json!({
        "status": "ok",
        "version": "1.16.1",
        "type": "navidrome",
        "serverVersion": "0.58.0 (b2ad9d0d)",
        "openSubsonic": true
    });
    if let (Some(target), Value::Object(extra)) = (body.as_object_mut(), extra) {
        target.extend(extra);
    }
    ResponseTemplate::new(200).set_body_json(json!({ "subsonic-response": body }))
}

fn subsonic_failed(code: i64, message: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "subsonic-response": {
            "status": "failed",
            "version": "1.16.1",
            "error": { "code": code, "message": message }
        }
    }))
}

async fn mount_ping(mock: &MockServer, version: &str, extensions: &[&str]) {
    Mock::given(method("GET"))
        .and(path("/rest/ping.view"))
        .respond_with(subsonic_ok(json!({ "serverVersion": version })))
        .mount(mock)
        .await;
    let extensions: Vec<Value> = extensions
        .iter()
        .map(|name| json!({ "name": name, "versions": [1] }))
        .collect();
    Mock::given(method("GET"))
        .and(path("/rest/getOpenSubsonicExtensions.view"))
        .respond_with(subsonic_ok(json!({ "openSubsonicExtensions": extensions })))
        .mount(mock)
        .await;
}

fn nd_song(id: &str, title: &str) -> Value {
    json!({
        "id": id,
        "title": title,
        "album": "Album One",
        "albumId": "al1",
        "artist": "Artist One",
        "artistId": "ar1",
        "albumArtist": "Artist One",
        "albumArtistId": "ar1",
        "participants": {
            "albumartist": [{ "id": "ar1", "name": "Artist One" }],
            "artist": [{ "id": "ar1", "name": "Artist One" }]
        },
        "bitRate": 320,
        "bookmarkPosition": 0,
        "compilation": false,
        "createdAt": "2024-03-01T10:00:00Z",
        "discNumber": 1,
        "duration": 245.3,
        "genre": "Rock",
        "genres": [{ "id": "g1", "name": "Rock" }],
        "hasCoverArt": true,
        "libraryPath": "/music",
        "orderAlbumArtistName": "artist one",
        "orderAlbumName": "album one",
        "orderArtistName": "artist one",
        "orderTitle": title.to_lowercase(),
        "path": "Artist One/Album One/01.flac",
        "playCount": 3,
        "playDate": "2024-05-01T10:00:00Z",
        "rating": 4,
        "rgTrackGain": -7.1,
        "rgTrackPeak": 0.98,
        "sampleRate": 44100,
        "size": 31_000_000,
        "starred": true,
        "suffix": "flac",
        "trackNumber": 1,
        "updatedAt": "2024-03-01T10:00:00Z",
        "year": 2001
    })
}

fn nd_album(id: &str) -> Value {
    json!({
        "id": id,
        "name": "Album One",
        "albumArtist": "Artist One",
        "albumArtistId": "ar1",
        "compilation": false,
        "createdAt": "2024-03-01T10:00:00Z",
        "duration": 2400.5,
        "externalInfoUpdatedAt": null,
        "genre": "Rock",
        "genres": [{ "id": "g1", "name": "Rock" }],
        "libraryId": 1,
        "libraryName": "Music",
        "libraryPath": "/music",
        "maxYear": 2001,
        "minYear": 2001,
        "orderAlbumArtistName": "artist one",
        "orderAlbumName": "album one",
        "releaseDate": "2001-06-01",
        "size": 300_000_000,
        "songCount": 10,
        "starred": false,
        "updatedAt": "2024-03-01T10:00:00Z"
    })
}

fn nd_playlist(id: &str, name: &str) -> Value {
    json!({
        "id": id,
        "name": name,
        "comment": "",
        "createdAt": "2024-01-01T00:00:00Z",
        "duration": 600.0,
        "evaluatedAt": null,
        "ownerId": "u1",
        "ownerName": "alice",
        "path": "",
        "public": false,
        "rules": null,
        "size": 1000,
        "songCount": 2,
        "sync": false,
        "updatedAt": "2024-02-02T00:00:00Z"
    })
}

fn query_values(request: &Request, key: &str) -> Vec<String> {
    request
        .url
        .query_pairs()
        .filter(|(k, _)| k == key)
        .map(|(_, v)| v.into_owned())
        .collect()
}

#[tokio::test]
async fn login_builds_navidrome_credential() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .and(body_json(
            json!({ "username": "alice", "password": "secret" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "u1",
            "isAdmin": false,
            "name": "Alice",
            "subsonicSalt": "salt1",
            "subsonicToken": "tok1",
            "token": "jwt-1",
            "username": "alice"
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let (server, credential) = NavidromeServer::login(config(&mock.uri()), "secret")
        .await
        .expect("login");

    assert_eq!(
        credential,
        Credential::Navidrome {
            subsonic: SUBSONIC_AUTH.into(),
            nd_token: Some("jwt-1".into()),
        }
    );
    assert_eq!(server.credential(), credential);
    assert_eq!(server.config().id, "srv1");
}

#[tokio::test]
async fn login_rejected_is_auth_error() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({ "error": "Invalid" })))
        .mount(&mock)
        .await;

    let result = NavidromeServer::login(config(&mock.uri()), "wrong").await;
    assert!(matches!(result, Err(ApiError::Auth(_))));
}

#[tokio::test]
async fn with_credential_rejects_other_backends() {
    let result = NavidromeServer::with_credential(
        config("http://localhost:1"),
        &Credential::Subsonic {
            subsonic: SUBSONIC_AUTH.into(),
        },
    );
    assert!(matches!(result, Err(ApiError::Auth(_))));
}

#[tokio::test]
async fn native_calls_pick_up_refreshed_token() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/song/s1"))
        .and(header("x-nd-authorization", "Bearer jwt-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-nd-authorization", "jwt-2")
                .set_body_json(nd_song("s1", "First")),
        )
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/song/s2"))
        .and(header("x-nd-authorization", "Bearer jwt-2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(nd_song("s2", "Second")))
        .expect(1)
        .mount(&mock)
        .await;

    let server = server(&mock, Some("jwt-1"));
    assert_eq!(server.song("s1").await.expect("s1").title, "First");
    assert_eq!(
        server.credential(),
        Credential::Navidrome {
            subsonic: SUBSONIC_AUTH.into(),
            nd_token: Some("jwt-2".into()),
        }
    );
    assert_eq!(server.song("s2").await.expect("s2").title, "Second");
}

#[tokio::test]
async fn song_list_sends_paging_sort_filters_and_reads_total() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.58.0 (b2ad9d0d)", &[]).await;
    Mock::given(method("GET"))
        .and(path("/api/song"))
        .and(query_param("_start", "20"))
        .and(query_param("_end", "70"))
        .and(query_param("_sort", "year"))
        .and(query_param("_order", "DESC"))
        .and(query_param("starred", "true"))
        .and(query_param("title", "love"))
        .and(query_param("missing", "false"))
        .and(query_param("library_id", "lib1"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-total-count", "1234")
                .set_body_json(json!([
                    nd_song("s1", "Love One"),
                    nd_song("s2", "Love Two")
                ])),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let server = server(&mock, Some("jwt-1"));
    let query = SongListQuery {
        sort: SongSort::Year,
        order: SortOrder::Desc,
        search: Some("love".into()),
        favorite: Some(true),
        genre_ids: vec!["g1".into(), "g2".into()],
        artist_ids: vec!["ar1".into()],
        music_folder_id: Some("lib1".into()),
        ..SongListQuery::default()
    };
    let page = server
        .song_list(
            &query,
            Paging {
                start_index: 20,
                limit: 50,
            },
        )
        .await
        .expect("song list");

    assert_eq!(page.start_index, 20);
    assert_eq!(page.total_count, Some(1234));
    assert_eq!(page.items.len(), 2);
    let song = &page.items[0];
    assert_eq!(song.id, "s1");
    assert_eq!(song.server_id, "srv1");
    assert_eq!(song.duration_ms, 245_300);
    assert_eq!(song.artists[0].name, "Artist One");
    assert_eq!(song.album_id.as_deref(), Some("al1"));
    assert_eq!(song.user_rating, Some(4));
    assert!(song.favorite);
    assert_eq!(
        song.path.as_deref(),
        Some("/music/Artist One/Album One/01.flac")
    );
    assert_eq!(song.container.as_deref(), Some("flac"));
    assert_eq!(song.cover_art_id.as_deref(), Some("s1"));
    assert_eq!(song.replay_gain.and_then(|r| r.track_gain_db), Some(-7.1));

    let requests = mock.received_requests().await.unwrap_or_default();
    let list = requests
        .iter()
        .find(|r| r.url.path() == "/api/song")
        .expect("song list request");
    assert_eq!(query_values(list, "genre_id"), vec!["g1", "g2"]);
    assert_eq!(query_values(list, "artists_id"), vec!["ar1"]);
    assert!(query_values(list, "album_artist_id").is_empty());
}

#[tokio::test]
async fn song_count_on_old_server_uses_legacy_filters() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.54.5", &[]).await;
    Mock::given(method("GET"))
        .and(path("/api/song"))
        .and(query_param("_start", "0"))
        .and(query_param("_end", "1"))
        .and(query_param("_sort", "title"))
        .and(query_param("_order", "ASC"))
        .and(query_param("album_artist_id", "ar1"))
        .and(query_param_is_missing("missing"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-total-count", "42")
                .set_body_json(json!([nd_song("s1", "One")])),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let server = server(&mock, Some("jwt-1"));
    let query = SongListQuery {
        artist_ids: vec!["ar1".into()],
        ..SongListQuery::default()
    };
    assert_eq!(server.song_count(&query).await.expect("count"), 42);
}

#[tokio::test]
async fn song_count_without_session_is_auth_error() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.58.0", &[]).await;
    let server = server(&mock, None);
    let result = server.song_count(&SongListQuery::default()).await;
    assert!(matches!(result, Err(ApiError::Auth(_))));
}

#[tokio::test]
async fn rejected_token_falls_back_to_subsonic() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/song/s1"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/getSong.view"))
        .and(query_param("id", "s1"))
        .and(query_param("u", "alice"))
        .and(query_param("s", "salt1"))
        .and(query_param("t", "tok1"))
        .and(query_param("v", "1.16.1"))
        .and(query_param("c", "feishin"))
        .and(query_param("f", "json"))
        .respond_with(subsonic_ok(json!({
            "song": {
                "id": "s1",
                "parent": "al1",
                "isDir": false,
                "title": "From Subsonic",
                "album": "Album One",
                "artist": "Artist One",
                "artistId": "ar1",
                "albumId": "al1",
                "coverArt": "mf-s1",
                "size": 100,
                "contentType": "audio/mpeg",
                "suffix": "mp3",
                "duration": 200,
                "path": "a/b.mp3",
                "created": "2024-01-01T00:00:00Z",
                "type": "music",
                "isVideo": false,
                "discNumber": 1,
                "track": 3,
                "artists": [{ "id": "ar1", "name": "Artist One" }],
                "albumArtists": [{ "id": "ar1", "name": "Artist One" }]
            }
        })))
        .expect(2)
        .mount(&mock)
        .await;

    let server = server(&mock, Some("expired"));
    let song = server.song("s1").await.expect("fallback song");
    assert_eq!(song.title, "From Subsonic");
    assert_eq!(song.cover_art_id.as_deref(), Some("mf-s1"));
    assert_eq!(song.track_number, Some(3));
    assert_eq!(
        server.credential(),
        Credential::Navidrome {
            subsonic: SUBSONIC_AUTH.into(),
            nd_token: None,
        }
    );
    server
        .song("s1")
        .await
        .expect("second call goes straight to subsonic");
}

#[tokio::test]
async fn album_detail_fetches_album_and_tracks() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.58.0", &[]).await;
    Mock::given(method("GET"))
        .and(path("/api/album/al1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(nd_album("al1")))
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
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!([nd_song("s1", "One"), nd_song("s2", "Two")])),
        )
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

#[tokio::test]
async fn artist_detail_combines_artist_albums_and_top_songs() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.58.0", &["topSongsByArtistId"]).await;
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
        .mount(&mock)
        .await;
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
                .set_body_json(json!([nd_album("al1"), nd_album("al2")])),
        )
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/getTopSongs.view"))
        .and(query_param("id", "ar1"))
        .respond_with(subsonic_ok(json!({
            "topSongs": { "song": [{
                "id": "s9", "title": "Hit", "isDir": false, "parent": "al1", "size": 1,
                "contentType": "audio/flac", "suffix": "flac", "path": "p", "created": "c",
                "type": "music", "isVideo": false, "discNumber": 1,
                "artists": [], "albumArtists": []
            }]}
        })))
        .expect(1)
        .mount(&mock)
        .await;

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

#[tokio::test]
async fn playlist_list_and_songs() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.58.0", &[]).await;
    Mock::given(method("GET"))
        .and(path("/api/playlist"))
        .and(query_param("_start", "0"))
        .and(query_param("_end", "25"))
        .and(query_param("_sort", "name"))
        .and(query_param("_order", "ASC"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-total-count", "3")
                .set_body_json(json!([nd_playlist("pl1", "Mix")])),
        )
        .expect(1)
        .mount(&mock)
        .await;
    let mut track = nd_song("1", "One");
    track["mediaFileId"] = json!("s1");
    track["playlistId"] = json!("pl1");
    Mock::given(method("GET"))
        .and(path("/api/playlist/pl1/tracks"))
        .and(query_param("_sort", "id"))
        .and(query_param("_end", "-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([track])))
        .expect(1)
        .mount(&mock)
        .await;

    let server = server(&mock, Some("jwt-1"));
    let page = server
        .playlist_list(Paging {
            start_index: 0,
            limit: 25,
        })
        .await
        .expect("playlists");
    assert_eq!(page.total_count, Some(3));
    let playlist = &page.items[0];
    assert_eq!(playlist.name, "Mix");
    assert_eq!(playlist.owner.as_deref(), Some("alice"));
    assert_eq!(playlist.comment, None);
    assert_eq!(playlist.duration_ms, 600_000);
    assert_eq!(
        playlist.cover_art_id.as_deref(),
        Some("pl1&_=2024-02-02T00:00:00Z")
    );

    let songs = server.playlist_songs("pl1").await.expect("tracks");
    assert_eq!(songs[0].id, "s1");
}

#[tokio::test]
async fn playlist_create_add_remove_delete() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.58.0", &[]).await;
    Mock::given(method("POST"))
        .and(path("/api/playlist"))
        .and(body_json(json!({ "name": "New", "rules": null })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "pl9" })))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/playlist/pl9/tracks"))
        .and(body_json(json!({ "ids": ["s1", "s2"] })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "added": 2 })))
        .expect(2)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/playlist/pl9"))
        .respond_with(ResponseTemplate::new(200).set_body_json(nd_playlist("pl9", "New")))
        .expect(1)
        .mount(&mock)
        .await;
    let mut first = nd_song("7", "One");
    first["mediaFileId"] = json!("s1");
    let mut second = nd_song("8", "Two");
    second["mediaFileId"] = json!("s2");
    Mock::given(method("GET"))
        .and(path("/api/playlist/pl9/tracks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([first, second])))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/playlist/pl9/tracks"))
        .and(query_param("id", "8"))
        .and(query_param_is_missing("_start"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ids": ["8"] })))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/playlist/pl9"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(null)))
        .expect(1)
        .mount(&mock)
        .await;

    let server = server(&mock, Some("jwt-1"));
    let ids = vec!["s1".to_owned(), "s2".to_owned()];
    let created = server.create_playlist("New", &ids).await.expect("create");
    assert_eq!(created.id, "pl9");
    assert_eq!(created.name, "New");
    server.add_to_playlist("pl9", &ids).await.expect("add");
    server
        .remove_from_playlist("pl9", &[1])
        .await
        .expect("remove");
    assert!(matches!(
        server.remove_from_playlist("pl9", &[]).await,
        Ok(())
    ));
    server.delete_playlist("pl9").await.expect("delete");
}

#[tokio::test]
async fn playlist_ops_without_session_use_subsonic() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.58.0", &[]).await;
    Mock::given(method("GET"))
        .and(path("/rest/updatePlaylist.view"))
        .and(query_param("playlistId", "pl1"))
        .and(query_param("songIndexToRemove", "2"))
        .respond_with(subsonic_ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/deletePlaylist.view"))
        .and(query_param("id", "pl1"))
        .respond_with(subsonic_ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;

    let server = server(&mock, None);
    server
        .remove_from_playlist("pl1", &[2])
        .await
        .expect("remove");
    server.delete_playlist("pl1").await.expect("delete");
}

#[tokio::test]
async fn favorites_and_rating_use_subsonic() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/star.view"))
        .and(query_param("albumId", "al1"))
        .and(query_param("albumId", "al2"))
        .and(query_param("u", "alice"))
        .respond_with(subsonic_ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/unstar.view"))
        .and(query_param("id", "s1"))
        .respond_with(subsonic_ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/setRating.view"))
        .and(query_param("id", "ar1"))
        .and(query_param("rating", "5"))
        .respond_with(subsonic_ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;

    let server = server(&mock, Some("jwt-1"));
    server
        .set_favorite(ItemKind::Album, &["al1".into(), "al2".into()], true)
        .await
        .expect("star");
    server
        .set_favorite(ItemKind::Song, &["s1".into()], false)
        .await
        .expect("unstar");
    server
        .set_rating(ItemKind::Artist, "ar1", 9)
        .await
        .expect("rating");
    assert!(matches!(
        server
            .set_favorite(ItemKind::Playlist, &["pl1".into()], true)
            .await,
        Err(ApiError::Unsupported(_))
    ));
}

#[tokio::test]
async fn scrobble_now_playing_and_submission() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/scrobble.view"))
        .and(query_param("id", "s1"))
        .and(query_param("submission", "false"))
        .and(query_param_is_missing("time"))
        .respond_with(subsonic_ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/scrobble.view"))
        .and(query_param("id", "s1"))
        .and(query_param("submission", "true"))
        .and(query_param("time", "1700000000000"))
        .respond_with(subsonic_ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;

    let server = server(&mock, Some("jwt-1"));
    server
        .scrobble("s1", false, None)
        .await
        .expect("now playing");
    server
        .scrobble("s1", true, Some(1_700_000_000_000))
        .await
        .expect("submission");
}

#[tokio::test]
async fn subsonic_errors_are_mapped() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/scrobble.view"))
        .respond_with(subsonic_failed(70, "not found"))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/ping.view"))
        .respond_with(subsonic_failed(40, "Wrong username or password"))
        .mount(&mock)
        .await;

    let server = server(&mock, None);
    assert!(matches!(
        server.scrobble("x", true, None).await,
        Err(ApiError::NotFound)
    ));
    assert!(matches!(server.ping().await, Err(ApiError::Auth(_))));
}

#[tokio::test]
async fn play_queue_prefers_opensubsonic_index_queue_for_saving() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.58.0", &["indexBasedQueue"]).await;
    Mock::given(method("GET"))
        .and(path("/rest/savePlayQueueByIndex.view"))
        .and(query_param("id", "s1"))
        .and(query_param("id", "s2"))
        .and(query_param("currentIndex", "1"))
        .and(query_param("position", "12345"))
        .respond_with(subsonic_ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/queue"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "q1",
            "userId": "u1",
            "current": 1,
            "position": 12345,
            "changedBy": "feishin",
            "createdAt": "2024-01-01T00:00:00Z",
            "updatedAt": "2024-01-01T00:00:00Z",
            "items": [nd_song("s1", "One"), nd_song("s2", "Two")]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let server = server(&mock, Some("jwt-1"));
    server
        .save_play_queue(&["s1".into(), "s2".into()], Some("s2"), 12_345)
        .await
        .expect("save");
    let queue = server.play_queue().await.expect("queue").expect("some");
    assert_eq!(queue.songs.len(), 2);
    assert_eq!(queue.current_song_id.as_deref(), Some("s2"));
    assert_eq!(queue.position_ms, 12_345);
}

#[tokio::test]
async fn play_queue_uses_native_api_without_index_extension() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.57.0", &[]).await;
    Mock::given(method("POST"))
        .and(path("/api/queue"))
        .and(body_json(
            json!({ "ids": ["s1"], "current": 0, "position": 5 }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(null)))
        .expect(1)
        .mount(&mock)
        .await;

    server(&mock, Some("jwt-1"))
        .save_play_queue(&["s1".into()], Some("s1"), 5)
        .await
        .expect("save");
}

#[tokio::test]
async fn play_queue_legacy_subsonic() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.50.0", &[]).await;
    Mock::given(method("GET"))
        .and(path("/rest/getPlayQueue.view"))
        .respond_with(subsonic_ok(json!({
            "playQueue": {
                "current": "s7",
                "position": 99,
                "username": "alice",
                "changed": "2024-01-01T00:00:00Z",
                "changedBy": "feishin",
                "entry": [{
                    "id": "s7", "title": "Seven", "isDir": false, "parent": "p", "size": 1,
                    "contentType": "audio/mpeg", "suffix": "mp3", "path": "p", "created": "c",
                    "type": "music", "isVideo": false, "discNumber": 1,
                    "artists": [], "albumArtists": []
                }]
            }
        })))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/savePlayQueue.view"))
        .and(query_param("current", "s7"))
        .and(query_param("id", "s7"))
        .respond_with(subsonic_ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;

    let server = server(&mock, Some("jwt-1"));
    let queue = server.play_queue().await.expect("queue").expect("some");
    assert_eq!(queue.current_song_id.as_deref(), Some("s7"));
    assert_eq!(queue.position_ms, 99);
    server
        .save_play_queue(&["s7".into()], Some("s7"), 99)
        .await
        .expect("save");
}

#[tokio::test]
async fn lyrics_from_opensubsonic_extension() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.58.0", &["songLyrics"]).await;
    let song_fixture = nd_song("s1", "One");
    Mock::given(method("GET"))
        .and(path("/api/song/s1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(song_fixture))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/getLyricsBySongId.view"))
        .and(query_param("id", "s1"))
        .respond_with(subsonic_ok(json!({
            "lyricsList": { "structuredLyrics": [{
                "displayArtist": "Artist One",
                "displayTitle": "One",
                "lang": "eng",
                "synced": true,
                "line": [{ "start": 1500, "value": "hello" }]
            }]}
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let server = server(&mock, Some("jwt-1"));
    let song = server.song("s1").await.expect("song");
    let lyrics = server.lyrics(&song).await.expect("lyrics");
    assert_eq!(lyrics.as_deref(), Some("[00:01.50]hello"));
}

#[tokio::test]
async fn search_random_and_similar_use_subsonic() {
    let mock = MockServer::start().await;
    let entry = |id: &str| {
        json!({
            "id": id, "title": id, "isDir": false, "parent": "p", "size": 1,
            "contentType": "audio/mpeg", "suffix": "mp3", "path": "p", "created": "c",
            "type": "music", "isVideo": false, "discNumber": 1,
            "artists": [], "albumArtists": []
        })
    };
    Mock::given(method("GET"))
        .and(path("/rest/search3.view"))
        .and(query_param("query", "abc"))
        .and(query_param("songCount", "5"))
        .respond_with(subsonic_ok(json!({
            "searchResult3": {
                "artist": [{ "id": "ar1", "name": "A", "albumCount": 2 }],
                "album": [{
                    "id": "al1", "name": "Al", "artist": "A", "artistId": "ar1", "coverArt": "al-1",
                    "songCount": 3, "duration": 100, "created": "c", "year": 1999
                }],
                "song": [entry("s1")]
            }
        })))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/getRandomSongs.view"))
        .and(query_param("size", "2"))
        .and(query_param("genre", "Rock"))
        .respond_with(subsonic_ok(
            json!({ "randomSongs": { "song": [entry("r1"), entry("r2")] } }),
        ))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/getSimilarSongs.view"))
        .and(query_param("id", "s1"))
        .and(query_param("count", "3"))
        .respond_with(subsonic_ok(
            json!({ "similarSongs": { "song": [entry("s1"), entry("x1")] } }),
        ))
        .expect(1)
        .mount(&mock)
        .await;

    let server = server(&mock, Some("jwt-1"));
    let results = server.search("abc", 5).await.expect("search");
    assert_eq!(results.artists[0].album_count, Some(2));
    assert_eq!(results.albums[0].album_artists[0].id, "ar1");
    assert_eq!(results.albums[0].year, Some(1999));
    assert_eq!(results.songs.len(), 1);
    assert_eq!(
        server
            .random_songs(2, Some("Rock"))
            .await
            .expect("random")
            .len(),
        2
    );
    let similar = server.similar_songs("s1", 3).await.expect("similar");
    assert_eq!(similar.len(), 1);
    assert_eq!(similar[0].id, "x1");
}

#[tokio::test]
async fn genres_use_tag_endpoint_on_new_servers() {
    let mock = MockServer::start().await;
    mount_ping(&mock, "0.58.0", &[]).await;
    Mock::given(method("GET"))
        .and(path("/api/tag"))
        .and(query_param("tag_name", "genre"))
        .and(query_param("_sort", "tagValue"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "id": "t1", "tagName": "genre", "tagValue": "Jazz", "albumCount": 2, "songCount": 20 }
        ])))
        .expect(1)
        .mount(&mock)
        .await;

    let genres = server(&mock, Some("jwt-1"))
        .genre_list()
        .await
        .expect("genres");
    assert_eq!(genres[0].id, "t1");
    assert_eq!(genres[0].name, "Jazz");
    assert_eq!(genres[0].song_count, Some(20));
}

#[tokio::test]
async fn url_builders_include_credentials() {
    let server = NavidromeServer::with_credential(
        config("https://music.example.com"),
        &credential(Some("jwt-1")),
    )
    .expect("server");
    let song = feishin_core::model::Song {
        id: "s 1".into(),
        server_id: "srv1".into(),
        title: "t".into(),
        artists: Vec::new(),
        album_artists: Vec::new(),
        album: None,
        album_id: None,
        duration_ms: 0,
        track_number: None,
        disc_number: None,
        year: None,
        genres: Vec::new(),
        bitrate_kbps: None,
        play_count: 0,
        user_rating: None,
        favorite: false,
        cover_art_id: None,
        replay_gain: None,
        path: None,
        size_bytes: None,
        container: None,
        created_at: None,
        last_played_at: None,
    };

    assert_eq!(
        server.stream_url(&song, &StreamOptions::default()),
        "https://music.example.com/rest/stream.view?u=alice&s=salt1&t=tok1&v=1.16.1&c=feishin&id=s+1"
    );
    assert_eq!(
        server.stream_url(
            &song,
            &StreamOptions {
                max_bitrate_kbps: Some(192),
                format: Some("opus".into()),
            }
        ),
        "https://music.example.com/rest/stream.view?u=alice&s=salt1&t=tok1&v=1.16.1&c=feishin&id=s+1&format=opus&maxBitRate=192"
    );
    assert_eq!(
        server.cover_art_url("al-1", Some(300)),
        "https://music.example.com/rest/getCoverArt.view?u=alice&s=salt1&t=tok1&v=1.16.1&c=feishin&id=al-1&size=300"
    );
    assert_eq!(
        server.cover_art_url("pl1&_=2024-02-02T00:00:00Z", None),
        "https://music.example.com/rest/getCoverArt.view?u=alice&s=salt1&t=tok1&v=1.16.1&c=feishin&id=pl1&_=2024-02-02T00%3A00%3A00Z"
    );
}
