use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use feishin_api::{Credential, MusicServer};
use feishin_core::model::ServerConfig;

use crate::demo::{DEMO_SONG_COUNT, FakeServer, demo_credential};

/// Creates `MusicServer` sessions, either by logging in or from a saved credential.
#[async_trait]
pub trait Connector: Send + Sync {
    async fn login(
        &self,
        config: ServerConfig,
        password: &str,
    ) -> Result<(Arc<dyn MusicServer>, Credential)>;

    fn restore(
        &self,
        config: &ServerConfig,
        credential: &Credential,
    ) -> Result<Arc<dyn MusicServer>>;
}

pub struct ServerConnector;

#[async_trait]
impl Connector for ServerConnector {
    async fn login(
        &self,
        config: ServerConfig,
        password: &str,
    ) -> Result<(Arc<dyn MusicServer>, Credential)> {
        use feishin_core::model::ServerType;
        Ok(match config.server_type {
            ServerType::Navidrome => {
                let (server, credential) =
                    feishin_api_navidrome::NavidromeServer::login(config, password).await?;
                (Arc::new(server) as Arc<dyn MusicServer>, credential)
            }
            ServerType::Subsonic => {
                let (server, credential) =
                    feishin_api_subsonic::SubsonicServer::login(config, password).await?;
                (Arc::new(server) as Arc<dyn MusicServer>, credential)
            }
            ServerType::Jellyfin => {
                let (server, credential) =
                    feishin_api_jellyfin::JellyfinServer::login(config, password).await?;
                (Arc::new(server) as Arc<dyn MusicServer>, credential)
            }
        })
    }

    fn restore(
        &self,
        config: &ServerConfig,
        credential: &Credential,
    ) -> Result<Arc<dyn MusicServer>> {
        use feishin_core::model::ServerType;
        let config = config.clone();
        Ok(match config.server_type {
            ServerType::Navidrome => Arc::new(
                feishin_api_navidrome::NavidromeServer::with_credential(config, credential)?,
            ),
            ServerType::Subsonic => Arc::new(
                feishin_api_subsonic::SubsonicServer::with_credential(config, credential)?,
            ),
            ServerType::Jellyfin => Arc::new(
                feishin_api_jellyfin::JellyfinServer::with_credential(config, credential)?,
            ),
        })
    }
}

/// Accepts any login and serves the generated demo library.
pub struct DemoConnector;

#[async_trait]
impl Connector for DemoConnector {
    async fn login(
        &self,
        config: ServerConfig,
        _password: &str,
    ) -> Result<(Arc<dyn MusicServer>, Credential)> {
        let server = FakeServer::with_config(config, DEMO_SONG_COUNT);
        Ok((Arc::new(server), demo_credential()))
    }

    fn restore(
        &self,
        config: &ServerConfig,
        _credential: &Credential,
    ) -> Result<Arc<dyn MusicServer>> {
        Ok(Arc::new(FakeServer::with_config(
            config.clone(),
            DEMO_SONG_COUNT,
        )))
    }
}
