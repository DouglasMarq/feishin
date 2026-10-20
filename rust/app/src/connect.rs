use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use feishin_api::{Credential, MusicServer};
use feishin_core::model::{ServerConfig, ServerType};

use crate::demo::{DemoLibrary, FakeServer, demo_credential};
use crate::lyrics_provider::{LyricsProvider, NoRemoteLyrics, RemoteLyrics};
use crate::radio_directory::{DemoDirectory, StationDirectory, public_directory};

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

    fn station_directory(&self) -> Arc<dyn StationDirectory> {
        public_directory()
    }

    fn lyrics(&self) -> Arc<dyn LyricsProvider>;
}

pub struct ServerConnector {
    lyrics: Arc<dyn LyricsProvider>,
}

impl ServerConnector {
    pub fn new() -> Result<Self> {
        Ok(Self {
            lyrics: Arc::new(RemoteLyrics::new()?),
        })
    }
}

#[async_trait]
impl Connector for ServerConnector {
    async fn login(
        &self,
        config: ServerConfig,
        password: &str,
    ) -> Result<(Arc<dyn MusicServer>, Credential)> {
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

    fn lyrics(&self) -> Arc<dyn LyricsProvider> {
        Arc::clone(&self.lyrics)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DemoConnector {
    library: DemoLibrary,
}

impl DemoConnector {
    pub const fn new(library: DemoLibrary) -> Self {
        Self { library }
    }
}

#[async_trait]
impl Connector for DemoConnector {
    async fn login(
        &self,
        config: ServerConfig,
        _password: &str,
    ) -> Result<(Arc<dyn MusicServer>, Credential)> {
        let server = FakeServer::with_config(config, self.library);
        Ok((Arc::new(server), demo_credential()))
    }

    fn restore(
        &self,
        config: &ServerConfig,
        _credential: &Credential,
    ) -> Result<Arc<dyn MusicServer>> {
        Ok(Arc::new(FakeServer::with_config(
            config.clone(),
            self.library,
        )))
    }

    fn station_directory(&self) -> Arc<dyn StationDirectory> {
        Arc::new(DemoDirectory)
    }

    fn lyrics(&self) -> Arc<dyn LyricsProvider> {
        Arc::new(NoRemoteLyrics)
    }
}
