use feishin_api::Credential;
use feishin_core::model::{ServerConfig, ServerType};

use super::server::FakeServer;

impl FakeServer {
    pub fn config_for_demo() -> ServerConfig {
        ServerConfig {
            id: "demo".into(),
            name: "Demo library".into(),
            server_type: ServerType::Navidrome,
            url: "demo://library".into(),
            username: "demo".into(),
        }
    }
}

pub fn demo_credential() -> Credential {
    Credential::Navidrome {
        subsonic: "u=demo".into(),
        nd_token: None,
    }
}
