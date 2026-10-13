use feishin_api::{Credential, MusicServer};
use feishin_api_subsonic::SubsonicServer;
use feishin_core::model::{Paging, ServerConfig, ServerType, Song};
use md5::{Digest, Md5};
use serde_json::{Value, json};
use wiremock::matchers::path;
use wiremock::{Match, Mock, MockServer, Request, ResponseTemplate};

pub const CREDENTIAL: &str = "u=alice&s=abc123&t=0123456789abcdef0123456789abcdef";

pub fn config(url: &str) -> ServerConfig {
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

pub fn ok(body: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(envelope("ok", body))
}

pub fn failed(code: Value, message: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(envelope(
        "failed",
        json!({ "error": { "code": code, "message": message } }),
    ))
}

pub fn song(id: &str, title: &str) -> Value {
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

pub fn album(id: &str, name: &str) -> Value {
    json!({
        "id": id, "name": name, "title": name, "album": name, "artist": "Artist",
        "artistId": "ar-1", "coverArt": format!("al-{id}"), "songCount": 10, "duration": 1800,
        "created": "2024-05-01T10:00:00Z", "year": 2020, "genre": "Rock", "isDir": true,
        "parent": "dir-1", "isVideo": false, "artists": [{ "id": "ar-1", "name": "Artist" }]
    })
}

pub fn artist(id: &str, name: &str, album_count: u32) -> Value {
    json!({ "id": id, "name": name, "albumCount": album_count, "coverArt": format!("ar-{id}") })
}

pub fn playlist(id: &str, name: &str) -> Value {
    json!({
        "id": id, "name": name, "comment": "c", "owner": "alice", "public": true, "songCount": 2,
        "duration": 360, "created": "2024-01-01T00:00:00Z", "changed": "2024-02-01T00:00:00Z",
        "coverArt": format!("pl-{id}")
    })
}

pub fn paging(start_index: usize, limit: usize) -> Paging {
    Paging { start_index, limit }
}

fn query_value(request: &Request, key: &str) -> Option<String> {
    request
        .url
        .query_pairs()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.into_owned())
}

pub struct AllQueryValues {
    pub key: &'static str,
    pub expected: Vec<&'static str>,
}

pub fn all_query_values(key: &'static str, expected: Vec<&'static str>) -> AllQueryValues {
    AllQueryValues { key, expected }
}

impl Match for AllQueryValues {
    fn matches(&self, request: &Request) -> bool {
        let values: Vec<String> = request
            .url
            .query_pairs()
            .filter(|(name, _)| name == self.key)
            .map(|(_, value)| value.into_owned())
            .collect();
        values == self.expected
    }
}

pub struct ValidSaltedToken {
    pub password: &'static str,
}

pub fn valid_salted_token(password: &'static str) -> ValidSaltedToken {
    ValidSaltedToken { password }
}

impl Match for ValidSaltedToken {
    fn matches(&self, request: &Request) -> bool {
        let (Some(salt), Some(token)) = (query_value(request, "s"), query_value(request, "t"))
        else {
            return false;
        };
        let expected: String = Md5::digest(format!("{}{salt}", self.password).as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        salt.len() == 12 && token == expected
    }
}

pub async fn mount_plain_ping(mock: &MockServer) {
    Mock::given(path("/rest/ping.view"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "subsonic-response": { "status": "ok", "version": "1.16.1" }
        })))
        .mount(mock)
        .await;
}

pub async fn mount_extensions(mock: &MockServer, extensions: Value) {
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

pub fn server(mock: &MockServer) -> SubsonicServer {
    let credential = Credential::Subsonic {
        subsonic: CREDENTIAL.into(),
    };
    match SubsonicServer::with_credential(config(&mock.uri()), &credential) {
        Ok(server) => server,
        Err(error) => panic!("with_credential failed: {error}"),
    }
}

pub fn offset_response(
    request: &Request,
    total: usize,
    offset_key: &str,
    count_key: &str,
) -> Vec<Value> {
    let numeric_value = |key: &str| {
        query_value(request, key)
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0)
    };
    let offset = numeric_value(offset_key);
    let count = numeric_value(count_key);
    (offset..total.min(offset + count))
        .map(|index| song(&format!("s{index}"), "x"))
        .collect()
}

pub fn server_song(server: &SubsonicServer) -> Song {
    Song {
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
