mod albums;
mod annotations;
mod artists;
mod auth;
mod counting;
mod library;
mod music_server;
mod pages;
mod play_queue;
mod playlists;
mod requests;
mod songs;
mod transport;
mod urls;

use feishin_api::Result;
use feishin_core::model::ServerConfig;
use tokio::sync::OnceCell;

use crate::client::{Client, Params, RequestEncoding};
use crate::extensions::Extensions;
use crate::types::{OpenSubsonicExtensions, Ping};

pub struct SubsonicServer {
    config: ServerConfig,
    client: Client,
    extensions: OnceCell<Extensions>,
}

fn is_open_subsonic(ping: &Ping) -> bool {
    ping.open_subsonic.unwrap_or(false) && ping.server_version.is_some()
}

impl SubsonicServer {
    fn new(config: ServerConfig, auth: String) -> Result<Self> {
        let client = Client::new(&config.url, auth)?;
        Ok(Self {
            config,
            client,
            extensions: OnceCell::new(),
        })
    }

    pub async fn extensions(&self) -> Result<&Extensions> {
        self.extensions
            .get_or_try_init(|| self.detect_extensions())
            .await
    }

    async fn detect_extensions(&self) -> Result<Extensions> {
        let ping: Ping = self
            .client
            .call("ping.view", &Params::new(), RequestEncoding::QueryString)
            .await?
            .decode()?;
        if !is_open_subsonic(&ping) {
            return Ok(Extensions::default());
        }
        let response: OpenSubsonicExtensions = self
            .client
            .call(
                "getOpenSubsonicExtensions.view",
                &Params::new(),
                RequestEncoding::QueryString,
            )
            .await?
            .decode()?;
        Ok(Extensions::from_response(response))
    }
}
