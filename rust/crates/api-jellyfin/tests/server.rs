use feishin_api::{ApiError, Credential, MusicServer, StreamOptions};
use feishin_api_jellyfin::{JellyfinServer, auth_header};
use feishin_core::model::{
    AlbumListQuery, AlbumSort, ArtistListQuery, ItemKind, Paging, ServerConfig, ServerType,
    SongListQuery, SongSort, SortOrder,
};
use serde_json::{Value, json};
use wiremock::matchers::{body_json, body_partial_json, method, path, query_param};
use wiremock::{Match, Mock, MockServer, Request, ResponseTemplate};

const USER: &str = "u1";
const TOKEN: &str = "tok123";

struct AuthHeader(String);

impl Match for AuthHeader {
    fn matches(&self, request: &Request) -> bool {
        request
            .headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value == self.0)
    }
}

fn authed() -> AuthHeader {
    AuthHeader(auth_header("srv1", Some(TOKEN)))
}

fn config(url: &str) -> ServerConfig {
    ServerConfig {
        id: "srv1".into(),
        name: "Jelly".into(),
        server_type: ServerType::Jellyfin,
        url: format!("{url}/"),
        username: "alice".into(),
    }
}

fn server(mock: &MockServer) -> JellyfinServer {
    let credential = Credential::Jellyfin {
        access_token: TOKEN.into(),
        user_id: USER.into(),
    };
    JellyfinServer::with_credential(config(&mock.uri()), &credential).unwrap()
}

fn user_data(favorite: bool, plays: u32) -> Value {
    json!({
        "IsFavorite": favorite,
        "Key": "k",
        "PlaybackPositionTicks": 0,
        "PlayCount": plays,
        "Played": plays > 0,
        "LastPlayedDate": "2024-02-03T04:05:06.0000000Z"
    })
}

fn song_json(id: &str, album_id: &str) -> Value {
    json!({
        "Album": "Album One",
        "AlbumArtist": "Artist A",
        "AlbumArtists": [{ "Id": "ar1", "Name": "Artist A" }],
        "AlbumId": album_id,
        "AlbumPrimaryImageTag": "albtag",
        "ArtistItems": [{ "Id": "ar2", "Name": "Artist B" }],
        "Artists": ["Artist B"],
        "BackdropImageTags": [],
        "ChannelId": null,
        "DateCreated": "2023-01-01T00:00:00.0000000Z",
        "ExternalUrls": [],
        "GenreItems": [{ "Id": "g1", "Name": "Rock" }],
        "Genres": ["Rock"],
        "Id": id,
        "ImageBlurHashes": {},
        "ImageTags": {},
        "IndexNumber": 3,
        "IsFolder": false,
        "LocationType": "FileSystem",
        "MediaSources": [{
            "Container": "flac",
            "Path": "/music/a/03.flac",
            "Size": 31_000_000,
            "MediaStreams": [
                { "Type": "Video", "Codec": "mjpeg", "BitRate": 1 },
                { "Type": "Audio", "Codec": "flac", "BitRate": 912_345, "SampleRate": 44100 }
            ]
        }],
        "MediaType": "Audio",
        "Name": format!("Song {id}"),
        "NormalizationGain": -6.5,
        "ParentIndexNumber": 2,
        "PlaylistItemId": format!("entry-{id}"),
        "PremiereDate": "2010-06-01T00:00:00.0000000Z",
        "ProductionYear": 2009,
        "RunTimeTicks": 2_455_000_000_u64,
        "ServerId": "s",
        "Type": "Audio",
        "UserData": user_data(true, 7)
    })
}

fn album_json(id: &str) -> Value {
    json!({
        "AlbumArtist": "Artist A",
        "AlbumArtists": [{ "Id": "ar1", "Name": "Artist A" }],
        "AlbumPrimaryImageTag": "x",
        "ArtistItems": [],
        "Artists": [],
        "ChannelId": null,
        "ChildCount": 12,
        "DateCreated": "2023-01-01T00:00:00.0000000Z",
        "ExternalUrls": [],
        "GenreItems": [{ "Id": "g1", "Name": "Rock" }],
        "Genres": ["Rock"],
        "Id": id,
        "ImageBlurHashes": {},
        "ImageTags": { "Primary": "primtag" },
        "IsFolder": true,
        "LocationType": "FileSystem",
        "Name": format!("Album {id}"),
        "ProductionYear": 1999,
        "RunTimeTicks": 36_000_000_000_u64,
        "ServerId": "s",
        "Studios": [],
        "Type": "MusicAlbum",
        "UserData": user_data(false, 3)
    })
}

fn artist_json(id: &str) -> Value {
    json!({
        "AlbumCount": 4,
        "BackdropImageTags": [],
        "ChannelId": null,
        "DateCreated": "2023-01-01T00:00:00.0000000Z",
        "ExternalUrls": [],
        "GenreItems": [],
        "Genres": [],
        "Id": id,
        "ImageBlurHashes": {},
        "ImageTags": { "Primary": "t" },
        "LocationType": "FileSystem",
        "Name": format!("Artist {id}"),
        "Overview": "Bio",
        "RunTimeTicks": 0,
        "ServerId": "s",
        "SongCount": 40,
        "Type": "MusicArtist",
        "UserData": user_data(true, 0)
    })
}

fn playlist_json(id: &str) -> Value {
    json!({
        "BackdropImageTags": [],
        "ChannelId": null,
        "ChildCount": 2,
        "DateCreated": "2024-01-01T00:00:00.0000000Z",
        "GenreItems": [],
        "Genres": [],
        "Id": id,
        "ImageBlurHashes": {},
        "ImageTags": {},
        "IsFolder": true,
        "LocationType": "FileSystem",
        "MediaType": "Audio",
        "Name": format!("Playlist {id}"),
        "Overview": "Mix",
        "RunTimeTicks": 4_910_000_000_u64,
        "ServerId": "s",
        "Type": "Playlist",
        "UserData": user_data(false, 0)
    })
}

fn list(items: Vec<Value>, total: usize) -> Value {
    json!({ "Items": items, "StartIndex": 0, "TotalRecordCount": total })
}

fn items_path() -> String {
    format!("/users/{USER}/items")
}

fn page(start_index: usize, limit: usize) -> Paging {
    Paging { start_index, limit }
}

#[tokio::test]
async fn login_sends_mediabrowser_header_and_returns_credential() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/users/authenticatebyname"))
        .and(AuthHeader(auth_header("srv1", None)))
        .and(body_json(json!({ "Pw": "secret", "Username": "alice" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "AccessToken": TOKEN,
            "ServerId": "s",
            "SessionInfo": {},
            "User": { "Id": USER, "Name": "alice", "Policy": { "IsAdministrator": false } }
        })))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/system/info"))
        .and(authed())
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "Version": "10.10.0" })))
        .expect(1)
        .mount(&mock)
        .await;

    let (server, credential) = JellyfinServer::login(config(&mock.uri()), "secret")
        .await
        .unwrap();
    assert_eq!(
        credential,
        Credential::Jellyfin {
            access_token: TOKEN.into(),
            user_id: USER.into()
        }
    );
    assert_eq!(server.user_id(), USER);
    server.ping().await.unwrap();
}

#[test]
fn auth_header_matches_feishin_format() {
    let header = auth_header("dev", Some("t"));
    assert!(header.starts_with(
        r#"MediaBrowser Client="Feishin", Device="Desktop Client", DeviceId="dev", Version=""#
    ));
    assert!(header.ends_with(r#"", Token="t""#));
    assert!(!auth_header("dev", None).contains("Token"));
}

#[tokio::test]
async fn login_failure_is_auth_error() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/users/authenticatebyname"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Invalid username or password"))
        .mount(&mock)
        .await;

    let result = JellyfinServer::login(config(&mock.uri()), "bad").await;
    assert!(matches!(result, Err(ApiError::Auth(_))));
}

#[test]
fn with_credential_rejects_other_backends() {
    let credential = Credential::Subsonic {
        subsonic: "u=a&p=b".into(),
    };
    assert!(JellyfinServer::with_credential(config("http://x"), &credential).is_err());
}

#[tokio::test]
async fn song_list_pages_sorts_filters_and_normalizes() {
    let mock = MockServer::start().await;
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
        .mount(&mock)
        .await;

    let query = SongListQuery {
        sort: SongSort::PlayCount,
        order: SortOrder::Desc,
        search: Some("foo".into()),
        favorite: Some(true),
        genre_ids: vec!["g1".into(), "g2".into()],
        year_from: Some(2000),
        year_to: Some(2002),
        music_folder_id: Some("lib1".into()),
        ..SongListQuery::default()
    };
    let result = server(&mock)
        .song_list(&query, page(100, 50))
        .await
        .unwrap();

    assert_eq!(result.start_index, 100);
    assert_eq!(result.total_count, Some(1234));
    assert_eq!(result.items.len(), 2);
    let song = &result.items[0];
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
        album_ids: (0..60).map(|i| format!("a{i}")).collect(),
        ..SongListQuery::default()
    };
    let result = server(&mock).song_list(&query, page(0, 500)).await.unwrap();
    assert_eq!(result.items.len(), 2);
    assert_eq!(result.total_count, Some(2));
}

#[tokio::test]
async fn album_list_and_count() {
    let mock = MockServer::start().await;
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
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path(items_path()))
        .and(query_param("IncludeItemTypes", "MusicAlbum"))
        .and(query_param("Limit", "1"))
        .and(query_param("StartIndex", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(vec![album_json("al1")], 321)))
        .expect(1)
        .mount(&mock)
        .await;

    let jf = server(&mock);
    let query = AlbumListQuery {
        sort: AlbumSort::RecentlyAdded,
        order: SortOrder::Desc,
        artist_ids: vec!["ar1".into()],
        ..AlbumListQuery::default()
    };
    let result = jf.album_list(&query, page(20, 10)).await.unwrap();
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
    assert_eq!(jf.album_count(&count_query).await.unwrap(), 321);
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

#[tokio::test]
async fn artist_detail_and_list() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/users/{USER}/items/ar1")))
        .and(query_param("Fields", "Overview"))
        .respond_with(ResponseTemplate::new(200).set_body_json(artist_json("ar1")))
        .expect(1)
        .mount(&mock)
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
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path(items_path()))
        .and(query_param("IncludeItemTypes", "Audio"))
        .and(query_param("ArtistIds", "ar1"))
        .and(query_param("SortBy", "CommunityRating,SortName"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(list(vec![song_json("s1", "al1")], 1)),
        )
        .expect(1)
        .mount(&mock)
        .await;
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
        .mount(&mock)
        .await;

    let jf = server(&mock);
    let detail = jf.artist("ar1").await.unwrap();
    assert_eq!(detail.artist.name, "Artist ar1");
    assert_eq!(detail.artist.album_count, Some(4));
    assert_eq!(detail.artist.song_count, Some(40));
    assert_eq!(detail.artist.biography.as_deref(), Some("Bio"));
    assert_eq!(detail.artist.cover_art_id.as_deref(), Some("ar1"));
    assert!(detail.artist.favorite);
    assert_eq!(detail.albums.len(), 2);
    assert_eq!(detail.top_songs.len(), 1);

    let query = ArtistListQuery {
        sort: feishin_core::model::ArtistSort::Random,
        favorite: Some(true),
        ..ArtistListQuery::default()
    };
    let artists = jf.artist_list(&query, page(5, 2)).await.unwrap();
    assert_eq!(artists.total_count, Some(77));
    assert_eq!(artists.items[1].id, "ar2");
}

#[tokio::test]
async fn genres_and_music_folders() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/musicgenres"))
        .and(query_param("UserId", USER))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(
            vec![json!({ "Id": "g1", "Name": "Rock", "ImageTags": {}, "ImageBlurHashes": {} })],
            1,
        )))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path(items_path()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "Items": [
            { "Id": "m1", "Name": "Music", "CollectionType": "music" },
            { "Id": "v1", "Name": "Movies", "CollectionType": "movies" },
            { "Id": "b1", "Name": "Books", "CollectionType": "books" }
        ] })))
        .mount(&mock)
        .await;

    let jf = server(&mock);
    let genres = jf.genre_list().await.unwrap();
    assert_eq!(genres[0].name, "Rock");
    let folders = jf.music_folders().await.unwrap();
    assert_eq!(folders.len(), 1);
    assert_eq!(folders[0].id, "m1");
}

#[tokio::test]
async fn random_and_similar_songs() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(items_path()))
        .and(query_param("SortBy", "Random,SortName"))
        .and(query_param("Limit", "5"))
        .and(query_param("GenreIds", "g1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(vec![song_json("s1", "a")], 1)))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/items/s1/similar"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(vec![song_json("s1", "a")], 1)))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/items/s1/InstantMix"))
        .and(query_param("Limit", "3"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(list(vec![song_json("s1", "a"), song_json("s9", "a")], 2)),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let jf = server(&mock);
    assert_eq!(jf.random_songs(5, Some("g1")).await.unwrap().len(), 1);
    let similar = jf.similar_songs("s1", 3).await.unwrap();
    assert_eq!(similar.len(), 1);
    assert_eq!(similar[0].id, "s9");
}

#[tokio::test]
async fn playlist_list_and_songs() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(items_path()))
        .and(query_param("IncludeItemTypes", "Playlist"))
        .and(query_param("MediaTypes", "Audio, Unknown"))
        .and(query_param("StartIndex", "0"))
        .and(query_param("Limit", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(vec![playlist_json("p1")], 3)))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/playlists/p1/items"))
        .and(query_param("UserId", USER))
        .and(query_param("IncludeItemTypes", "Audio"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(list(vec![song_json("s1", "a"), song_json("s2", "a")], 2)),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let jf = server(&mock);
    let playlists = jf.playlist_list(page(0, 25)).await.unwrap();
    assert_eq!(playlists.total_count, Some(3));
    let playlist = &playlists.items[0];
    assert_eq!(playlist.name, "Playlist p1");
    assert_eq!(playlist.comment.as_deref(), Some("Mix"));
    assert_eq!(playlist.song_count, 2);
    assert_eq!(playlist.duration_ms, 491_000);
    assert_eq!(playlist.cover_art_id, None);

    let songs = jf.playlist_songs("p1").await.unwrap();
    assert_eq!(songs.len(), 2);
}

#[tokio::test]
async fn create_playlist_adds_songs_and_returns_detail() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/playlists"))
        .and(authed())
        .and(body_json(
            json!({ "MediaType": "Audio", "Name": "New", "UserId": USER }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "Id": "p9" })))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/playlists/p9/items"))
        .and(query_param("Ids", "s1,s2"))
        .and(query_param("UserId", USER))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/users/{USER}/items/p9")))
        .and(query_param("Ids", "p9"))
        .respond_with(ResponseTemplate::new(200).set_body_json(playlist_json("p9")))
        .expect(1)
        .mount(&mock)
        .await;

    let playlist = server(&mock)
        .create_playlist("New", &["s1".into(), "s2".into()])
        .await
        .unwrap();
    assert_eq!(playlist.id, "p9");
}

#[tokio::test]
async fn add_to_playlist_batches_ids() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/playlists/p1/items"))
        .and(authed())
        .respond_with(ResponseTemplate::new(204))
        .expect(3)
        .mount(&mock)
        .await;

    let ids: Vec<String> = (0..120).map(|i| format!("s{i}")).collect();
    server(&mock).add_to_playlist("p1", &ids).await.unwrap();
}

#[tokio::test]
async fn remove_from_playlist_maps_indexes_to_entry_ids() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/playlists/p1/items"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(
            vec![
                song_json("s1", "a"),
                song_json("s2", "a"),
                song_json("s3", "a"),
            ],
            3,
        )))
        .mount(&mock)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/playlists/p1/items"))
        .and(query_param("EntryIds", "entry-s1,entry-s3"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock)
        .await;

    let jf = server(&mock);
    jf.remove_from_playlist("p1", &[0, 2]).await.unwrap();
    assert!(matches!(
        jf.remove_from_playlist("p1", &[9]).await,
        Err(ApiError::NotFound)
    ));
}

#[tokio::test]
async fn delete_playlist() {
    let mock = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/items/p1"))
        .and(authed())
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock)
        .await;

    server(&mock).delete_playlist("p1").await.unwrap();
}

#[tokio::test]
async fn favorites_and_rating() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/users/{USER}/favoriteitems/s1")))
        .and(authed())
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "IsFavorite": true })))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("/users/{USER}/favoriteitems/s2")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "IsFavorite": true })))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("DELETE"))
        .and(path(format!("/users/{USER}/favoriteitems/al1")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "IsFavorite": false })))
        .expect(1)
        .mount(&mock)
        .await;

    let jf = server(&mock);
    jf.set_favorite(ItemKind::Song, &["s1".into(), "s2".into()], true)
        .await
        .unwrap();
    jf.set_favorite(ItemKind::Album, &["al1".into()], false)
        .await
        .unwrap();
    assert!(matches!(
        jf.set_rating(ItemKind::Song, "s1", 4).await,
        Err(ApiError::Unsupported(_))
    ));
}

#[tokio::test]
async fn scrobble_reports_sessions() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/sessions/playing"))
        .and(authed())
        .and(body_json(json!({ "ItemId": "s1", "PositionTicks": 0 })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/users/{USER}/items/s1")))
        .respond_with(ResponseTemplate::new(200).set_body_json(song_json("s1", "a")))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/sessions/playing/stopped"))
        .and(body_json(json!({
            "ItemId": "s1",
            "IsPaused": true,
            "PositionTicks": 2_455_000_000_u64
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/sessions/playing/progress"))
        .and(body_partial_json(json!({
            "ItemId": "s1",
            "EventName": "pause",
            "IsPaused": true,
            "PositionTicks": 12_340_000
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock)
        .await;

    let jf = server(&mock);
    jf.scrobble("s1", false, None).await.unwrap();
    jf.scrobble("s1", true, Some(1_700_000_000_000))
        .await
        .unwrap();
    jf.report_progress("s1", 1234, true).await.unwrap();
}

#[tokio::test]
async fn play_queue_is_unsupported() {
    let mock = MockServer::start().await;
    let jf = server(&mock);
    assert!(matches!(
        jf.play_queue().await,
        Err(ApiError::Unsupported(_))
    ));
    assert!(matches!(
        jf.save_play_queue(&[], None, 0).await,
        Err(ApiError::Unsupported(_))
    ));
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

    let jf = server(&mock);
    let mut song = sample_song("s1");
    assert_eq!(
        jf.lyrics(&song).await.unwrap().as_deref(),
        Some("[00:01.00]Hello\n[00:02.50]World")
    );
    song.id = "s2".into();
    assert_eq!(jf.lyrics(&song).await.unwrap(), None);
}

#[tokio::test]
async fn unauthorized_maps_to_auth_error() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/users/{USER}/items/s1")))
        .respond_with(ResponseTemplate::new(401))
        .mount(&mock)
        .await;

    assert!(matches!(
        server(&mock).song("s1").await,
        Err(ApiError::Auth(_))
    ));
}

#[tokio::test]
async fn search_queries_albums_artists_and_songs() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(items_path()))
        .and(query_param("IncludeItemTypes", "MusicAlbum"))
        .and(query_param("SearchTerm", "abc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(vec![album_json("al1")], 1)))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/artists/albumArtists"))
        .and(query_param("SearchTerm", "abc"))
        .and(query_param("IncludeArtists", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(vec![artist_json("ar1")], 1)))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path(items_path()))
        .and(query_param("IncludeItemTypes", "Audio"))
        .and(query_param("SearchTerm", "abc"))
        .and(query_param("Limit", "7"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list(vec![song_json("s1", "a")], 1)))
        .expect(1)
        .mount(&mock)
        .await;

    let results = server(&mock).search("abc", 7).await.unwrap();
    assert_eq!(results.albums.len(), 1);
    assert_eq!(results.artists.len(), 1);
    assert_eq!(results.songs.len(), 1);
}

fn sample_song(id: &str) -> feishin_core::model::Song {
    feishin_core::model::Song {
        id: id.into(),
        server_id: "srv1".into(),
        title: "t".into(),
        artists: vec![],
        album_artists: vec![],
        album: None,
        album_id: None,
        duration_ms: 0,
        track_number: None,
        disc_number: None,
        year: None,
        genres: vec![],
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
    let credential = Credential::Jellyfin {
        access_token: TOKEN.into(),
        user_id: USER.into(),
    };
    let jf = JellyfinServer::with_credential(config("https://jf.example"), &credential).unwrap();
    let song = sample_song("s1");

    assert_eq!(
        jf.stream_url(&song, &StreamOptions::default()),
        "https://jf.example/Items/s1/Download?apiKey=tok123&playSessionId="
    );
    assert_eq!(
        jf.stream_url(
            &song,
            &StreamOptions {
                max_bitrate_kbps: Some(192),
                format: Some("opus".into()),
            }
        ),
        "https://jf.example/audio/s1/universal?userId=u1&deviceId=&audioCodec=opus&apiKey=tok123\
         &playSessionId=feishin-s1-opus-0-0&container=opus&transcodingProtocol=http\
         &transcodingContainer=opus&maxStreamingBitrate=192000"
    );
    assert_eq!(
        jf.stream_url(
            &song,
            &StreamOptions {
                max_bitrate_kbps: Some(128),
                format: None,
            }
        ),
        "https://jf.example/audio/s1/universal?userId=u1&deviceId=&audioCodec=mp3&apiKey=tok123\
         &playSessionId=feishin-s1-mp3-0-0&container=mp3&transcodingProtocol=http\
         &transcodingContainer=mp3&maxStreamingBitrate=128000"
    );
    assert_eq!(
        jf.cover_art_url("al1", Some(300)),
        "https://jf.example/Items/al1/Images/Primary?quality=96&fillHeight=300&fillWidth=300"
    );
    assert_eq!(
        jf.cover_art_url("al1", None),
        "https://jf.example/Items/al1/Images/Primary?quality=96"
    );
}
