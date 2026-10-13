use feishin_core::model::{ServerConfig, ServerType};
use feishin_player::AudioDevice;
use feishin_storage::settings::Settings;

use crate::demo::FakeServer;

pub(super) fn device(name: &str, description: &str) -> AudioDevice {
    AudioDevice {
        name: name.into(),
        description: description.into(),
    }
}

pub(super) fn server(id: &str) -> ServerConfig {
    ServerConfig {
        id: id.into(),
        name: format!("Server {id}"),
        server_type: ServerType::Navidrome,
        url: format!("https://{id}.example.com"),
        username: "me".into(),
    }
}

pub(super) fn two_servers() -> Settings {
    Settings {
        servers: vec![FakeServer::config_for_demo(), server("b")],
        current_server_id: Some("demo".into()),
        ..Settings::default()
    }
}
