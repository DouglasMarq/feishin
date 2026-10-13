//! Subsonic backend for `feishin_api::MusicServer`.

use feishin_api::Result;
use feishin_core::model::ServerConfig;

pub struct SubsonicServer {
    _config: ServerConfig,
}

impl SubsonicServer {
    pub async fn login(_config: ServerConfig, _password: &str) -> Result<(Self, String)> {
        todo!()
    }

    pub fn with_credential(_config: ServerConfig, _credential: &str) -> Result<Self> {
        todo!()
    }
}
