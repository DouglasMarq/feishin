use feishin_api::Credential;
use feishin_api_navidrome::NavidromeServer;
use feishin_core::model::{ServerConfig, ServerType};
use serde_json::{Value, json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

pub const SUBSONIC_AUTH: &str = "u=alice&s=salt1&t=tok1";

pub fn config(url: &str) -> ServerConfig {
    ServerConfig {
        id: "srv1".into(),
        name: "Home".into(),
        server_type: ServerType::Navidrome,
        url: format!("{url}/"),
        username: "alice".into(),
    }
}

pub fn credential(token: Option<&str>) -> Credential {
    Credential::Navidrome {
        subsonic: SUBSONIC_AUTH.into(),
        nd_token: token.map(str::to_owned),
    }
}

pub fn server(mock: &MockServer, token: Option<&str>) -> NavidromeServer {
    NavidromeServer::with_credential(config(&mock.uri()), &credential(token)).expect("server")
}

pub fn merged(base: Value, extra: Value) -> Value {
    let mut merged = base;
    if let (Some(target), Value::Object(extra)) = (merged.as_object_mut(), extra) {
        target.extend(extra);
    }
    merged
}

pub fn subsonic_ok(extra: Value) -> ResponseTemplate {
    let body = merged(
        json!({
            "status": "ok",
            "version": "1.16.1",
            "type": "navidrome",
            "serverVersion": "0.58.0 (b2ad9d0d)",
            "openSubsonic": true
        }),
        extra,
    );
    ResponseTemplate::new(200).set_body_json(json!({ "subsonic-response": body }))
}

pub fn subsonic_failed(code: i64, message: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "subsonic-response": {
            "status": "failed",
            "version": "1.16.1",
            "error": { "code": code, "message": message }
        }
    }))
}

pub async fn mount_ping(mock: &MockServer, version: &str, extensions: &[&str]) {
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

pub fn nd_song(id: &str, title: &str) -> Value {
    merged(nd_song_identity(id, title), nd_song_metadata())
}

fn nd_song_identity(id: &str, title: &str) -> Value {
    json!({
        "id": id,
        "title": title,
        "orderTitle": title.to_lowercase(),
        "album": "Album One",
        "albumId": "al1",
        "artist": "Artist One",
        "artistId": "ar1",
        "albumArtist": "Artist One",
        "albumArtistId": "ar1",
        "participants": {
            "albumartist": [{ "id": "ar1", "name": "Artist One" }],
            "artist": [{ "id": "ar1", "name": "Artist One" }]
        }
    })
}

fn nd_song_metadata() -> Value {
    json!({
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

pub fn nd_album(id: &str) -> Value {
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

pub fn nd_playlist(id: &str, name: &str) -> Value {
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

#[derive(Debug, Clone, Copy)]
pub enum AudioFormat {
    Flac,
    Mp3,
}

impl AudioFormat {
    fn content_type_and_suffix(self) -> (&'static str, &'static str) {
        match self {
            Self::Flac => ("audio/flac", "flac"),
            Self::Mp3 => ("audio/mpeg", "mp3"),
        }
    }
}

pub fn subsonic_entry(id: &str, title: &str, parent: &str, format: AudioFormat) -> Value {
    let (content_type, suffix) = format.content_type_and_suffix();
    json!({
        "id": id, "title": title, "isDir": false, "parent": parent, "size": 1,
        "contentType": content_type, "suffix": suffix, "path": "p", "created": "c",
        "type": "music", "isVideo": false, "discNumber": 1,
        "artists": [], "albumArtists": []
    })
}

pub fn query_values(request: &Request, key: &str) -> Vec<String> {
    request
        .url
        .query_pairs()
        .filter(|(name, _)| name == key)
        .map(|(_, value)| value.into_owned())
        .collect()
}
