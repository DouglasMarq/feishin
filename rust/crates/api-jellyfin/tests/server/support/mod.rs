mod fixtures;

use feishin_api::Credential;
use feishin_api_jellyfin::{JellyfinServer, auth_header};
use feishin_core::model::{Paging, ServerConfig, ServerType};
use wiremock::{Match, MockServer, Request};

pub use fixtures::{album_json, artist_json, list, playlist_json, sample_song, song_json};

pub const USER: &str = "u1";
pub const TOKEN: &str = "tok123";

pub struct AuthHeader(pub String);

impl Match for AuthHeader {
    fn matches(&self, request: &Request) -> bool {
        request
            .headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value == self.0)
    }
}

pub fn authed() -> AuthHeader {
    AuthHeader(auth_header("srv1", Some(TOKEN)))
}

pub fn config(url: &str) -> ServerConfig {
    ServerConfig {
        id: "srv1".into(),
        name: "Jelly".into(),
        server_type: ServerType::Jellyfin,
        url: format!("{url}/"),
        username: "alice".into(),
    }
}

pub fn server_at(url: &str) -> JellyfinServer {
    let credential = Credential::Jellyfin {
        access_token: TOKEN.into(),
        user_id: USER.into(),
    };
    JellyfinServer::with_credential(config(url), &credential).unwrap()
}

pub fn server(mock: &MockServer) -> JellyfinServer {
    server_at(&mock.uri())
}

pub fn items_path() -> String {
    format!("/users/{USER}/items")
}

pub fn page(start_index: usize, limit: usize) -> Paging {
    Paging { start_index, limit }
}
