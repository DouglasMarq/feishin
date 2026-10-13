use feishin_api::{ApiError, Credential, MusicServer, StreamOptions};
use feishin_api_subsonic::SubsonicServer;
use feishin_core::model::{
    AlbumListQuery, AlbumSort, ArtistListQuery, ArtistSort, ItemKind, Paging, ServerConfig,
    ServerType, SongListQuery, SongSort, SortOrder,
};
use md5::{Digest, Md5};
use serde_json::{Value, json};
use wiremock::matchers::{body_string_contains, method, path, query_param, query_param_is_missing};
use wiremock::{Match, Mock, MockServer, Request, ResponseTemplate};

const CREDENTIAL: &str = "u=alice&s=abc123&t=0123456789abcdef0123456789abcdef";

fn config(url: &str) -> ServerConfig {
    ServerConfig {
        id: "srv-1".into(),
        name: "Test".into(),
        server_type: ServerType::Subsonic,
        url: url.into(),
        username: "alice".into(),
    }
}

fn envelope(status: &str, body: Value) -> Value {
    let mut inner = json!({
        "status": status,
        "version": "1.16.1",
        "type": "generic",
        "serverVersion": "1.0.0",
        "openSubsonic": true
    });
    if let (Some(inner), Value::Object(body)) = (inner.as_object_mut(), body) {
        inner.extend(body);
    }
    json!({ "subsonic-response": inner })
}

fn ok(body: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(envelope("ok", body))
}

fn failed(code: Value, message: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(envelope(
        "failed",
        json!({ "error": { "code": code, "message": message } }),
    ))
}

fn song(id: &str, title: &str) -> Value {
    json!({
        "id": id, "parent": "dir-1", "isDir": false, "title": title, "album": "Album",
        "albumId": "al-1", "artist": "Artist", "artistId": "ar-1", "track": 1, "discNumber": 1,
        "year": 2020, "genre": "Rock", "coverArt": format!("mf-{id}"), "size": 4_000_000,
        "contentType": "audio/mpeg", "suffix": "mp3", "duration": 180, "bitRate": 320,
        "path": format!("Artist/Album/{title}.mp3"), "created": "2024-05-01T10:00:00Z",
        "type": "music", "isVideo": false, "playCount": 3,
        "artists": [{ "id": "ar-1", "name": "Artist" }],
        "albumArtists": [{ "id": "ar-1", "name": "Artist" }]
    })
}

fn album(id: &str, name: &str) -> Value {
    json!({
        "id": id, "name": name, "title": name, "album": name, "artist": "Artist",
        "artistId": "ar-1", "coverArt": format!("al-{id}"), "songCount": 10, "duration": 1800,
        "created": "2024-05-01T10:00:00Z", "year": 2020, "genre": "Rock", "isDir": true,
        "parent": "dir-1", "isVideo": false, "artists": [{ "id": "ar-1", "name": "Artist" }]
    })
}

fn artist(id: &str, name: &str, album_count: u32) -> Value {
    json!({ "id": id, "name": name, "albumCount": album_count, "coverArt": format!("ar-{id}") })
}

fn playlist(id: &str, name: &str) -> Value {
    json!({
        "id": id, "name": name, "comment": "c", "owner": "alice", "public": true, "songCount": 2,
        "duration": 360, "created": "2024-01-01T00:00:00Z", "changed": "2024-02-01T00:00:00Z",
        "coverArt": format!("pl-{id}")
    })
}

fn paging(start_index: usize, limit: usize) -> Paging {
    Paging { start_index, limit }
}

/// Every value of `key` in the query string, in order.
struct QueryAll(&'static str, Vec<&'static str>);

impl Match for QueryAll {
    fn matches(&self, request: &Request) -> bool {
        let values: Vec<String> = request
            .url
            .query_pairs()
            .filter(|(k, _)| k == self.0)
            .map(|(_, v)| v.into_owned())
            .collect();
        values == self.1
    }
}

/// Valid `t = md5(password + s)` token auth.
struct Token(&'static str);

impl Match for Token {
    fn matches(&self, request: &Request) -> bool {
        let get = |key: &str| {
            request
                .url
                .query_pairs()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.into_owned())
        };
        let (Some(salt), Some(token)) = (get("s"), get("t")) else {
            return false;
        };
        let expected: String = Md5::digest(format!("{}{salt}", self.0).as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        salt.len() == 12 && token == expected
    }
}

async fn mount_plain_ping(mock: &MockServer) {
    Mock::given(path("/rest/ping.view"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "subsonic-response": { "status": "ok", "version": "1.16.1" }
        })))
        .mount(mock)
        .await;
}

async fn mount_extensions(mock: &MockServer, extensions: Value) {
    Mock::given(path("/rest/ping.view"))
        .respond_with(ok(json!({})))
        .mount(mock)
        .await;
    Mock::given(path("/rest/getOpenSubsonicExtensions.view"))
        .respond_with(ok(json!({ "openSubsonicExtensions": extensions })))
        .expect(1)
        .mount(mock)
        .await;
}

fn server(mock: &MockServer) -> SubsonicServer {
    let credential = Credential::Subsonic {
        subsonic: CREDENTIAL.into(),
    };
    match SubsonicServer::with_credential(config(&mock.uri()), &credential) {
        Ok(server) => server,
        Err(e) => panic!("with_credential failed: {e}"),
    }
}

#[tokio::test]
async fn login_uses_token_auth() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/getUser.view"))
        .and(query_param("u", "alice"))
        .and(query_param("username", "alice"))
        .and(query_param("v", "1.13.0"))
        .and(query_param("c", "Feishin"))
        .and(query_param("f", "json"))
        .and(query_param_is_missing("p"))
        .and(Token("s3cret"))
        .respond_with(ok(
            json!({ "user": { "username": "alice", "adminRole": false } }),
        ))
        .expect(1)
        .mount(&mock)
        .await;

    let (server, credential) = SubsonicServer::login(config(&mock.uri()), "s3cret")
        .await
        .expect("login");
    let Credential::Subsonic { subsonic } = &credential else {
        panic!("unexpected credential {credential:?}");
    };
    assert!(subsonic.starts_with("u=alice&s="));
    assert!(subsonic.contains("&t="));
    assert_eq!(server.config().username, "alice");

    let restored = SubsonicServer::with_credential(config(&mock.uri()), &credential);
    assert!(restored.is_ok());
}

#[tokio::test]
async fn login_falls_back_to_plain_password() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/getUser.view"))
        .and(Token("pa ss&word"))
        .respond_with(failed(json!(41), "Token authentication not supported"))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(path("/rest/getUser.view"))
        .and(query_param("p", "pa ss&word"))
        .and(query_param_is_missing("t"))
        .respond_with(ok(json!({ "user": { "username": "alice" } })))
        .expect(1)
        .mount(&mock)
        .await;

    let (_, credential) = SubsonicServer::login(config(&mock.uri()), "pa ss&word")
        .await
        .expect("login");
    assert_eq!(
        credential,
        Credential::Subsonic {
            subsonic: "u=alice&p=pa%20ss%26word".into()
        }
    );
}

#[tokio::test]
async fn login_with_wrong_password_is_auth_error() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/getUser.view"))
        .respond_with(failed(json!(40), "Wrong username or password"))
        .expect(2)
        .mount(&mock)
        .await;

    let result = SubsonicServer::login(config(&mock.uri()), "nope").await;
    assert!(matches!(result, Err(ApiError::Auth(m)) if m == "Wrong username or password"));
}

#[test]
fn with_credential_validates_input() {
    let cfg = config("http://localhost");
    let bad = Credential::Subsonic {
        subsonic: "u=alice".into(),
    };
    assert!(matches!(
        SubsonicServer::with_credential(cfg.clone(), &bad),
        Err(ApiError::Auth(_))
    ));
    let jellyfin = Credential::Jellyfin {
        access_token: "x".into(),
        user_id: "y".into(),
    };
    assert!(matches!(
        SubsonicServer::with_credential(cfg.clone(), &jellyfin),
        Err(ApiError::Auth(_))
    ));
    let legacy = Credential::Subsonic {
        subsonic: "u=alice&p=pw".into(),
    };
    assert!(SubsonicServer::with_credential(cfg, &legacy).is_ok());
}

#[tokio::test]
async fn error_envelope_maps_to_api_errors() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/getSong.view"))
        .and(query_param("id", "server"))
        .respond_with(failed(json!(10), "Required parameter is missing"))
        .mount(&mock)
        .await;
    Mock::given(path("/rest/getSong.view"))
        .and(query_param("id", "auth"))
        .respond_with(failed(json!("40"), "Wrong username or password"))
        .mount(&mock)
        .await;
    Mock::given(path("/rest/getSong.view"))
        .and(query_param("id", "missing"))
        .respond_with(failed(json!(70), "Song not found"))
        .mount(&mock)
        .await;
    Mock::given(path("/rest/getSong.view"))
        .and(query_param("id", "soft"))
        .respond_with(failed(json!(0), "generic"))
        .mount(&mock)
        .await;
    Mock::given(path("/rest/getSong.view"))
        .and(query_param("id", "http"))
        .respond_with(ResponseTemplate::new(502).set_body_string("Bad Gateway"))
        .mount(&mock)
        .await;
    Mock::given(path("/rest/getSimilarSongs.view"))
        .respond_with(failed(json!(0), "Last.fm not linked"))
        .mount(&mock)
        .await;
    let server = server(&mock);

    assert!(matches!(
        server.song("server").await,
        Err(ApiError::Server { code: 10, message }) if message == "Required parameter is missing"
    ));
    assert!(matches!(server.song("auth").await, Err(ApiError::Auth(_))));
    assert!(matches!(
        server.song("missing").await,
        Err(ApiError::NotFound)
    ));
    assert!(matches!(
        server.song("soft").await,
        Err(ApiError::Server { code: 0, .. })
    ));
    assert!(matches!(
        server.song("http").await,
        Err(ApiError::Server { code: 502, .. })
    ));
    assert_eq!(server.similar_songs("x", 5).await.ok(), Some(Vec::new()));
}

#[tokio::test]
async fn requests_carry_credential_and_ping_works() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/ping.view"))
        .and(query_param("u", "alice"))
        .and(query_param("s", "abc123"))
        .and(query_param("t", "0123456789abcdef0123456789abcdef"))
        .and(query_param("v", "1.13.0"))
        .and(query_param("c", "Feishin"))
        .and(query_param("f", "json"))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    assert!(server(&mock).ping().await.is_ok());
}

#[tokio::test]
async fn song_list_without_filters_uses_empty_search3() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/search3.view"))
        .and(query_param("query", ""))
        .and(query_param("songCount", "2"))
        .and(query_param("songOffset", "100"))
        .and(query_param("albumCount", "0"))
        .and(query_param("artistCount", "0"))
        .and(query_param("musicFolderId", "mf-1"))
        .respond_with(ok(json!({
            "searchResult3": { "song": [song("s1", "One"), song("s2", "Two")] }
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let query = SongListQuery {
        music_folder_id: Some("mf-1".into()),
        ..Default::default()
    };
    let page = server(&mock)
        .song_list(&query, paging(100, 2))
        .await
        .expect("song list");
    assert_eq!(page.start_index, 100);
    assert_eq!(page.total_count, None);
    assert_eq!(page.items.len(), 2);
    let first = &page.items[0];
    assert_eq!(first.id, "s1");
    assert_eq!(first.server_id, "srv-1");
    assert_eq!(first.title, "One");
    assert_eq!(first.album.as_deref(), Some("Album"));
    assert_eq!(first.album_id.as_deref(), Some("al-1"));
    assert_eq!(first.artists[0].name, "Artist");
    assert_eq!(first.duration_ms, 180_000);
    assert_eq!(first.bitrate_kbps, Some(320));
    assert_eq!(first.container.as_deref(), Some("mpeg"));
    assert_eq!(first.cover_art_id.as_deref(), Some("mf-s1"));
    assert_eq!(first.play_count, 3);
    assert!(!first.favorite);
}

#[tokio::test]
async fn song_list_search_genre_and_random() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/search3.view"))
        .and(query_param("query", "love"))
        .and(query_param("songCount", "10"))
        .and(query_param("songOffset", "20"))
        .respond_with(ok(
            json!({ "searchResult3": { "song": [song("s1", "Love")] } }),
        ))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(path("/rest/getSongsByGenre.view"))
        .and(query_param("genre", "Rock"))
        .and(query_param("count", "10"))
        .and(query_param("offset", "20"))
        .respond_with(ok(
            json!({ "songsByGenre": { "song": [song("s2", "Rock")] } }),
        ))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(path("/rest/getRandomSongs.view"))
        .and(query_param("size", "10"))
        .and(query_param("genre", "Jazz"))
        .and(query_param("fromYear", "1990"))
        .respond_with(ok(
            json!({ "randomSongs": { "song": [song("s3", "Rand")] } }),
        ))
        .expect(1)
        .mount(&mock)
        .await;
    let server = server(&mock);

    let search = SongListQuery {
        search: Some("love".into()),
        genre_ids: vec!["Rock".into()],
        ..Default::default()
    };
    let page = server
        .song_list(&search, paging(20, 10))
        .await
        .expect("search");
    assert_eq!(page.items[0].id, "s1");

    let genre = SongListQuery {
        genre_ids: vec!["Rock".into()],
        ..Default::default()
    };
    let page = server
        .song_list(&genre, paging(20, 10))
        .await
        .expect("genre");
    assert_eq!(page.items[0].id, "s2");
    assert_eq!(page.start_index, 20);

    let random = SongListQuery {
        sort: SongSort::Random,
        genre_ids: vec!["Jazz".into()],
        year_from: Some(1990),
        ..Default::default()
    };
    let page = server
        .song_list(&random, paging(0, 10))
        .await
        .expect("random");
    assert_eq!(page.items[0].id, "s3");
}

#[tokio::test]
async fn song_list_favorites_are_filtered_sorted_and_paged() {
    let mock = MockServer::start().await;
    let mut other = song("s4", "Other");
    other["albumArtists"] = json!([{ "id": "ar-9", "name": "Nobody" }]);
    let starred = |id: &str, title: &str| {
        let mut s = song(id, title);
        s["starred"] = json!("2024-06-01T00:00:00Z");
        s
    };
    Mock::given(path("/rest/getStarred2.view"))
        .respond_with(ok(json!({
            "starred2": {
                "song": [starred("s1", "charlie"), starred("s2", "Alpha"), starred("s3", "bravo"), other],
                "album": [], "artist": []
            }
        })))
        .expect(2)
        .mount(&mock)
        .await;
    let server = server(&mock);

    let query = SongListQuery {
        favorite: Some(true),
        sort: SongSort::Name,
        order: SortOrder::Desc,
        artist_ids: vec!["ar-1".into()],
        ..Default::default()
    };
    let page = server
        .song_list(&query, paging(1, 1))
        .await
        .expect("favorites");
    assert_eq!(page.total_count, Some(3));
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].title, "bravo");
    assert!(page.items[0].favorite);
    assert_eq!(server.song_count(&query).await.ok(), Some(3));
}

#[tokio::test]
async fn song_list_by_album_and_artist_ids() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/getArtist.view"))
        .and(query_param("id", "ar-1"))
        .respond_with(ok(json!({
            "artist": { "id": "ar-1", "name": "Artist", "albumCount": 1, "album": [album("al-2", "Second")] }
        })))
        .expect(2)
        .mount(&mock)
        .await;
    let mut first = album("al-1", "First");
    first["song"] = json!([song("s1", "One"), song("s2", "Two")]);
    Mock::given(path("/rest/getAlbum.view"))
        .and(query_param("id", "al-1"))
        .respond_with(ok(json!({ "album": first })))
        .mount(&mock)
        .await;
    let mut second = album("al-2", "Second");
    second["song"] = json!([song("s3", "Three")]);
    Mock::given(path("/rest/getAlbum.view"))
        .and(query_param("id", "al-2"))
        .respond_with(ok(json!({ "album": second })))
        .mount(&mock)
        .await;
    let server = server(&mock);

    let query = SongListQuery {
        album_ids: vec!["al-1".into()],
        artist_ids: vec!["ar-1".into()],
        ..Default::default()
    };
    let page = server
        .song_list(&query, paging(0, 0))
        .await
        .expect("by ids");
    let ids: Vec<&str> = page.items.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["s1", "s2", "s3"]);
    assert_eq!(page.total_count, Some(3));
    assert_eq!(server.song_count(&query).await.ok(), Some(3));
}

fn offset_response(
    request: &Request,
    total: usize,
    offset_key: &str,
    count_key: &str,
) -> Vec<Value> {
    let get = |key: &str| {
        request
            .url
            .query_pairs()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| v.parse::<usize>().ok())
            .unwrap_or(0)
    };
    let offset = get(offset_key);
    let count = get(count_key);
    (offset..total.min(offset + count))
        .map(|i| song(&format!("s{i}"), "x"))
        .collect()
}

#[tokio::test]
async fn song_count_probes_then_pages() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/search3.view"))
        .and(query_param("query", ""))
        .respond_with(|request: &Request| {
            let songs = offset_response(request, 12_345, "songOffset", "songCount");
            ok(json!({ "searchResult3": { "song": songs } }))
        })
        .expect(3 + 6)
        .mount(&mock)
        .await;
    Mock::given(path("/rest/getSongsByGenre.view"))
        .and(query_param("genre", "Rock"))
        .respond_with(|request: &Request| {
            let songs = offset_response(request, 1_000, "offset", "count");
            ok(json!({ "songsByGenre": { "song": songs } }))
        })
        .mount(&mock)
        .await;
    Mock::given(path("/rest/search3.view"))
        .and(query_param("query", "x"))
        .respond_with(|request: &Request| {
            let songs = offset_response(request, 742, "songOffset", "songCount");
            ok(json!({ "searchResult3": { "song": songs } }))
        })
        .mount(&mock)
        .await;
    let server = server(&mock);

    assert_eq!(
        server.song_count(&SongListQuery::default()).await.ok(),
        Some(12_345)
    );
    let genre = SongListQuery {
        genre_ids: vec!["Rock".into()],
        ..Default::default()
    };
    assert_eq!(server.song_count(&genre).await.ok(), Some(1_000));
    let search = SongListQuery {
        search: Some("x".into()),
        ..Default::default()
    };
    assert_eq!(server.song_count(&search).await.ok(), Some(742));
}

#[tokio::test]
async fn album_list_maps_sort_and_reads_total_header() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/getAlbumList2.view"))
        .and(query_param("type", "newest"))
        .and(query_param("size", "2"))
        .and(query_param("offset", "4"))
        .respond_with(
            ok(json!({ "albumList2": { "album": [album("a1", "One"), album("a2", "Two")] } }))
                .insert_header("x-total-count", "57"),
        )
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(path("/rest/getAlbumList2.view"))
        .and(query_param("type", "byGenre"))
        .and(query_param("genre", "Rock"))
        .respond_with(ok(
            json!({ "albumList2": { "album": [album("a3", "Three")] } }),
        ))
        .expect(1)
        .mount(&mock)
        .await;
    let server = server(&mock);

    let query = AlbumListQuery {
        sort: AlbumSort::RecentlyAdded,
        ..Default::default()
    };
    let page = server
        .album_list(&query, paging(4, 2))
        .await
        .expect("albums");
    assert_eq!(page.total_count, Some(57));
    assert_eq!(page.start_index, 4);
    let a = &page.items[0];
    assert_eq!(a.id, "a1");
    assert_eq!(a.name, "One");
    assert_eq!(a.song_count, 10);
    assert_eq!(a.duration_ms, 1_800_000);
    assert_eq!(a.year, Some(2020));
    assert_eq!(a.album_artists[0].id, "ar-1");
    assert_eq!(a.cover_art_id.as_deref(), Some("al-a1"));

    let genre = AlbumListQuery {
        genre_ids: vec!["Rock".into()],
        ..Default::default()
    };
    let page = server
        .album_list(&genre, paging(0, 50))
        .await
        .expect("genre");
    assert_eq!(page.items[0].id, "a3");
    assert_eq!(page.total_count, None);
}

#[tokio::test]
async fn album_list_favorites_and_search() {
    let mock = MockServer::start().await;
    let mut fav = album("a2", "Beta");
    fav["starred"] = json!("2024-01-01T00:00:00Z");
    Mock::given(path("/rest/getStarred2.view"))
        .respond_with(ok(json!({
            "starred2": { "album": [album("a1", "gamma"), fav, album("a3", "Alpha")] }
        })))
        .mount(&mock)
        .await;
    Mock::given(path("/rest/search3.view"))
        .and(query_param("query", "be"))
        .and(query_param("albumCount", "5"))
        .and(query_param("albumOffset", "0"))
        .and(query_param("songCount", "0"))
        .respond_with(ok(
            json!({ "searchResult3": { "album": [album("a2", "Beta")] } }),
        ))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(path("/rest/search3.view"))
        .and(query_param("query", "be"))
        .and(query_param("albumCount", "500"))
        .and(query_param("albumOffset", "0"))
        .respond_with(ok(
            json!({ "searchResult3": { "album": [album("a2", "Beta")] } }),
        ))
        .expect(1)
        .mount(&mock)
        .await;
    let server = server(&mock);

    let query = AlbumListQuery {
        favorite: Some(true),
        sort: AlbumSort::Name,
        ..Default::default()
    };
    let page = server
        .album_list(&query, paging(0, 2))
        .await
        .expect("favorites");
    let names: Vec<&str> = page.items.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["Alpha", "Beta"]);
    assert_eq!(page.total_count, Some(3));
    assert!(page.items[1].favorite);
    assert_eq!(server.album_count(&query).await.ok(), Some(3));

    let search = AlbumListQuery {
        search: Some("be".into()),
        ..Default::default()
    };
    let page = server
        .album_list(&search, paging(0, 5))
        .await
        .expect("search");
    assert_eq!(page.items.len(), 1);
    assert_eq!(server.album_count(&search).await.ok(), Some(1));
}

#[tokio::test]
async fn album_count_pages_without_total_header() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/getAlbumList2.view"))
        .and(query_param("type", "alphabeticalByName"))
        .and(query_param("size", "500"))
        .respond_with(|request: &Request| {
            let albums: Vec<Value> = offset_response(request, 1_203, "offset", "size")
                .iter()
                .map(|_| album("a", "x"))
                .collect();
            ok(json!({ "albumList2": { "album": albums } }))
        })
        .expect(3)
        .mount(&mock)
        .await;
    assert_eq!(
        server(&mock)
            .album_count(&AlbumListQuery::default())
            .await
            .ok(),
        Some(1_203)
    );
}

#[tokio::test]
async fn album_detail_includes_songs() {
    let mock = MockServer::start().await;
    let mut detail = album("a1", "One");
    detail["song"] = json!([song("s1", "First"), song("s2", "Second")]);
    detail["releaseDate"] = json!({ "year": 2019, "month": 5, "day": 3 });
    detail["userRating"] = json!(4);
    Mock::given(path("/rest/getAlbum.view"))
        .and(query_param("id", "a1"))
        .respond_with(ok(json!({ "album": detail })))
        .mount(&mock)
        .await;

    let detail = server(&mock).album("a1").await.expect("album");
    assert_eq!(detail.album.year, Some(2019));
    assert_eq!(detail.album.user_rating, Some(4));
    assert_eq!(detail.songs.len(), 2);
    assert_eq!(detail.songs[1].title, "Second");
}

#[tokio::test]
async fn artist_list_filters_sorts_and_pages_client_side() {
    let mock = MockServer::start().await;
    let mut starred = artist("ar-3", "Charlie", 7);
    starred["starred"] = json!("2024-01-01T00:00:00Z");
    Mock::given(path("/rest/getArtists.view"))
        .and(query_param("musicFolderId", "1"))
        .respond_with(ok(json!({
            "artists": {
                "ignoredArticles": "The",
                "index": [
                    { "name": "A", "artist": [artist("ar-1", "Alpha", 2), artist("ar-2", "Amber", 9)] },
                    { "name": "C", "artist": [starred] }
                ]
            }
        })))
        .mount(&mock)
        .await;
    let server = server(&mock);

    let query = ArtistListQuery {
        sort: ArtistSort::AlbumCount,
        order: SortOrder::Desc,
        music_folder_id: Some("1".into()),
        ..Default::default()
    };
    let page = server
        .artist_list(&query, paging(0, 2))
        .await
        .expect("artists");
    let names: Vec<&str> = page.items.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["Amber", "Charlie"]);
    assert_eq!(page.total_count, Some(3));
    assert_eq!(page.items[0].album_count, Some(9));

    let search = ArtistListQuery {
        search: Some("AL".into()),
        music_folder_id: Some("1".into()),
        ..Default::default()
    };
    let page = server
        .artist_list(&search, paging(0, 10))
        .await
        .expect("search");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, "ar-1");

    let favorites = ArtistListQuery {
        favorite: Some(true),
        music_folder_id: Some("1".into()),
        ..Default::default()
    };
    let page = server
        .artist_list(&favorites, paging(0, 10))
        .await
        .expect("favorites");
    assert_eq!(page.items.len(), 1);
    assert!(page.items[0].favorite);
}

#[tokio::test]
async fn artist_detail_fetches_top_songs_and_biography() {
    let mock = MockServer::start().await;
    mount_plain_ping(&mock).await;
    Mock::given(path("/rest/getArtist.view"))
        .and(query_param("id", "ar-1"))
        .respond_with(ok(json!({
            "artist": { "id": "ar-1", "name": "The Artist", "albumCount": 1, "album": [album("a1", "One")] }
        })))
        .mount(&mock)
        .await;
    Mock::given(path("/rest/getTopSongs.view"))
        .and(query_param("artist", "The Artist"))
        .respond_with(ok(json!({ "topSongs": { "song": [song("s1", "Hit")] } })))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(path("/rest/getArtistInfo2.view"))
        .and(query_param("id", "ar-1"))
        .respond_with(ok(json!({ "artistInfo2": { "biography": "Bio" } })))
        .mount(&mock)
        .await;

    let detail = server(&mock).artist("ar-1").await.expect("artist");
    assert_eq!(detail.artist.name, "The Artist");
    assert_eq!(detail.artist.biography.as_deref(), Some("Bio"));
    assert_eq!(detail.albums.len(), 1);
    assert_eq!(detail.top_songs[0].title, "Hit");
}

#[tokio::test]
async fn genres_and_music_folders() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/getGenres.view"))
        .respond_with(ok(json!({
            "genres": { "genre": [
                { "value": "rock", "songCount": 10, "albumCount": 2 },
                { "value": "Ambient", "songCount": 3, "albumCount": 1 }
            ] }
        })))
        .mount(&mock)
        .await;
    Mock::given(path("/rest/getMusicFolders.view"))
        .respond_with(ok(json!({
            "musicFolders": { "musicFolder": [{ "id": 1, "name": "Music" }] }
        })))
        .mount(&mock)
        .await;
    let server = server(&mock);

    let genres = server.genre_list().await.expect("genres");
    assert_eq!(genres[0].name, "Ambient");
    assert_eq!(genres[1].id, "rock");
    assert_eq!(genres[1].song_count, Some(10));
    let folders = server.music_folders().await.expect("folders");
    assert_eq!(folders[0].id, "1");
    assert_eq!(folders[0].name, "Music");
}

#[tokio::test]
async fn playlists_crud() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/getPlaylists.view"))
        .respond_with(ok(json!({
            "playlists": { "playlist": [playlist("p1", "One"), playlist("p2", "Two"), playlist("p3", "Three")] }
        })))
        .mount(&mock)
        .await;
    let mut detail = playlist("p1", "One");
    detail["entry"] = json!([song("s1", "A"), song("s2", "B")]);
    Mock::given(path("/rest/getPlaylist.view"))
        .and(query_param("id", "p1"))
        .respond_with(ok(json!({ "playlist": detail })))
        .mount(&mock)
        .await;
    Mock::given(path("/rest/createPlaylist.view"))
        .and(query_param("name", "New list"))
        .and(QueryAll("songId", vec!["s1", "s2"]))
        .respond_with(ok(json!({ "playlist": playlist("p9", "New list") })))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(path("/rest/updatePlaylist.view"))
        .and(query_param("playlistId", "p1"))
        .and(QueryAll("songIdToAdd", vec!["s3", "s4"]))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(path("/rest/updatePlaylist.view"))
        .and(query_param("playlistId", "p1"))
        .and(QueryAll("songIndexToRemove", vec!["0", "2"]))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(path("/rest/deletePlaylist.view"))
        .and(query_param("id", "p1"))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    let server = server(&mock);

    let page = server.playlist_list(paging(1, 1)).await.expect("playlists");
    assert_eq!(page.total_count, Some(3));
    let p = &page.items[0];
    assert_eq!(p.id, "p2");
    assert_eq!(p.owner.as_deref(), Some("alice"));
    assert!(p.public);
    assert_eq!(p.duration_ms, 360_000);
    assert_eq!(p.updated_at.as_deref(), Some("2024-02-01T00:00:00Z"));
    assert_eq!(p.cover_art_id.as_deref(), Some("pl-p2"));

    let songs = server.playlist_songs("p1").await.expect("songs");
    assert_eq!(songs.len(), 2);

    let created = server
        .create_playlist("New list", &["s1".into(), "s2".into()])
        .await
        .expect("create");
    assert_eq!(created.id, "p9");

    assert!(
        server
            .add_to_playlist("p1", &["s3".into(), "s4".into()])
            .await
            .is_ok()
    );
    assert!(server.remove_from_playlist("p1", &[0, 2]).await.is_ok());
    assert!(server.delete_playlist("p1").await.is_ok());
}

#[tokio::test]
async fn favorites_and_rating() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/star.view"))
        .and(QueryAll("albumId", vec!["a1", "a2"]))
        .and(query_param_is_missing("id"))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(path("/rest/unstar.view"))
        .and(QueryAll("id", vec!["s1"]))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(path("/rest/star.view"))
        .and(QueryAll("artistId", vec!["ar-1"]))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(path("/rest/setRating.view"))
        .and(query_param("id", "s1"))
        .and(query_param("rating", "5"))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(path("/rest/setRating.view"))
        .and(query_param("id", "a1"))
        .and(query_param("rating", "0"))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    let server = server(&mock);

    assert!(
        server
            .set_favorite(ItemKind::Album, &["a1".into(), "a2".into()], true)
            .await
            .is_ok()
    );
    assert!(
        server
            .set_favorite(ItemKind::Song, &["s1".into()], false)
            .await
            .is_ok()
    );
    assert!(
        server
            .set_favorite(ItemKind::Artist, &["ar-1".into()], true)
            .await
            .is_ok()
    );
    assert!(server.set_favorite(ItemKind::Song, &[], true).await.is_ok());
    assert!(matches!(
        server
            .set_favorite(ItemKind::Playlist, &["p1".into()], true)
            .await,
        Err(ApiError::Unsupported(_))
    ));

    assert!(server.set_rating(ItemKind::Song, "s1", 9).await.is_ok());
    assert!(server.set_rating(ItemKind::Album, "a1", 0).await.is_ok());
    assert!(matches!(
        server.set_rating(ItemKind::Playlist, "p1", 3).await,
        Err(ApiError::Unsupported(_))
    ));
}

#[tokio::test]
async fn scrobble_now_playing_and_submission() {
    let mock = MockServer::start().await;
    Mock::given(path("/rest/scrobble.view"))
        .and(query_param("id", "s1"))
        .and(query_param("submission", "false"))
        .and(query_param_is_missing("time"))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(path("/rest/scrobble.view"))
        .and(query_param("id", "s1"))
        .and(query_param("submission", "true"))
        .and(query_param("time", "1700000000000"))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    let server = server(&mock);

    assert!(
        server
            .scrobble("s1", false, Some(1_700_000_000_000))
            .await
            .is_ok()
    );
    assert!(
        server
            .scrobble("s1", true, Some(1_700_000_000_000))
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn play_queue_on_plain_subsonic() {
    let mock = MockServer::start().await;
    mount_plain_ping(&mock).await;
    Mock::given(path("/rest/getOpenSubsonicExtensions.view"))
        .respond_with(ok(json!({})))
        .expect(0)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/savePlayQueue.view"))
        .and(QueryAll("id", vec!["s1", "s2", "s3"]))
        .and(query_param("current", "s2"))
        .and(query_param("position", "42000"))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(path("/rest/getPlayQueue.view"))
        .respond_with(ok(json!({
            "playQueue": {
                "current": "s2", "position": 42000, "username": "alice",
                "changed": "2024-01-01T00:00:00Z", "changedBy": "Feishin",
                "entry": [song("s1", "A"), song("s2", "B")]
            }
        })))
        .expect(1)
        .mount(&mock)
        .await;
    let server = server(&mock);

    let ids: Vec<String> = vec!["s1".into(), "s2".into(), "s3".into()];
    assert!(
        server
            .save_play_queue(&ids, Some("s2"), 42_000)
            .await
            .is_ok()
    );
    let queue = server
        .play_queue()
        .await
        .expect("queue")
        .expect("some queue");
    assert_eq!(queue.songs.len(), 2);
    assert_eq!(queue.current_song_id.as_deref(), Some("s2"));
    assert_eq!(queue.position_ms, 42_000);
    assert_eq!(server.extensions().await.ok(), Some(&Default::default()));
}

#[tokio::test]
async fn play_queue_by_index_with_form_post() {
    let mock = MockServer::start().await;
    mount_extensions(
        &mock,
        json!([
            { "name": "indexBasedQueue", "versions": [1] },
            { "name": "formPost", "versions": [1] }
        ]),
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/rest/savePlayQueueByIndex.view"))
        .and(body_string_contains("u=alice&s=abc123"))
        .and(body_string_contains(
            "&id=s1&id=s2&position=1500&currentIndex=1",
        ))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/getPlayQueueByIndex.view"))
        .respond_with(ok(json!({
            "playQueueByIndex": {
                "currentIndex": 1, "position": 1500, "username": "alice",
                "changed": "2024-01-01T00:00:00Z", "changedBy": "Feishin",
                "entry": [song("s1", "A"), song("s2", "B")]
            }
        })))
        .expect(1)
        .mount(&mock)
        .await;
    let server = server(&mock);

    let ids: Vec<String> = vec!["s1".into(), "s2".into()];
    assert!(
        server
            .save_play_queue(&ids, Some("s2"), 1_500)
            .await
            .is_ok()
    );
    let queue = server
        .play_queue()
        .await
        .expect("queue")
        .expect("some queue");
    assert_eq!(queue.current_song_id.as_deref(), Some("s2"));
    assert_eq!(queue.position_ms, 1_500);
}

#[tokio::test]
async fn empty_play_queue_is_none() {
    let mock = MockServer::start().await;
    mount_extensions(
        &mock,
        json!([{ "name": "indexBasedQueue", "versions": [1] }]),
    )
    .await;
    Mock::given(path("/rest/getPlayQueueByIndex.view"))
        .respond_with(ok(json!({})))
        .mount(&mock)
        .await;
    assert_eq!(server(&mock).play_queue().await.ok(), Some(None));
}

#[tokio::test]
async fn lyrics_use_song_lyrics_extension() {
    let mock = MockServer::start().await;
    mount_extensions(&mock, json!([{ "name": "songLyrics", "versions": [1, 2] }])).await;
    Mock::given(path("/rest/getLyricsBySongId.view"))
        .and(query_param("id", "s1"))
        .and(query_param("enhanced", "true"))
        .respond_with(ok(json!({
            "lyricsList": { "structuredLyrics": [
                { "displayArtist": "Artist", "displayTitle": "One", "lang": "eng", "synced": true,
                  "offset": 100, "line": [ { "start": 0, "value": "Hello" }, { "start": 2500, "value": "World" } ] }
            ] }
        })))
        .expect(1)
        .mount(&mock)
        .await;
    let server = server(&mock);
    let song = server_song(&server);

    let lyrics = server.lyrics(&song).await.expect("lyrics");
    assert_eq!(
        lyrics.as_deref(),
        Some("[offset:100]\n[00:00.00]Hello\n[00:02.50]World")
    );
}

#[tokio::test]
async fn lyrics_without_extension_skip_request() {
    let mock = MockServer::start().await;
    mount_plain_ping(&mock).await;
    Mock::given(path("/rest/getLyricsBySongId.view"))
        .respond_with(ok(json!({})))
        .expect(0)
        .mount(&mock)
        .await;
    let server = server(&mock);
    let song = server_song(&server);
    assert_eq!(server.lyrics(&song).await.ok(), Some(None));
}

fn server_song(server: &SubsonicServer) -> feishin_core::model::Song {
    feishin_core::model::Song {
        id: "s1".into(),
        server_id: server.config().id.clone(),
        title: "One".into(),
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
    }
}

#[test]
fn url_builders() {
    let credential = Credential::Subsonic {
        subsonic: CREDENTIAL.into(),
    };
    let server = SubsonicServer::with_credential(config("https://music.example.com/"), &credential)
        .expect("server");
    let mut song = server_song(&server);
    song.id = "a b/c".into();

    assert_eq!(
        server.stream_url(&song, &StreamOptions::default()),
        format!(
            "https://music.example.com/rest/stream.view?id=a%20b%2Fc&v=1.13.0&c=Feishin&{CREDENTIAL}"
        )
    );
    let options = StreamOptions {
        max_bitrate_kbps: Some(192),
        format: Some("opus".into()),
    };
    assert_eq!(
        server.stream_url(&song, &options),
        format!(
            "https://music.example.com/rest/stream.view?id=a%20b%2Fc&v=1.13.0&c=Feishin&{CREDENTIAL}&format=opus&maxBitRate=192"
        )
    );
    assert_eq!(
        server.cover_art_url("al-1", Some(300)),
        format!(
            "https://music.example.com/rest/getCoverArt.view?id=al-1&{CREDENTIAL}&v=1.13.0&c=Feishin&size=300"
        )
    );
    assert_eq!(
        server.cover_art_url("al-1", None),
        format!(
            "https://music.example.com/rest/getCoverArt.view?id=al-1&{CREDENTIAL}&v=1.13.0&c=Feishin"
        )
    );
}
