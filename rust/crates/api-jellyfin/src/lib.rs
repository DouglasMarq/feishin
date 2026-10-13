//! Jellyfin backend for `feishin_api::MusicServer`.

use feishin_api::{Credential, Result};
use feishin_core::model::ServerConfig;

pub struct JellyfinServer {
    _config: ServerConfig,
}

impl JellyfinServer {
    pub async fn login(_config: ServerConfig, _password: &str) -> Result<(Self, Credential)> {
        todo!()
    }

    pub fn with_credential(_config: ServerConfig, _credential: &Credential) -> Result<Self> {
        todo!()
    }
}
