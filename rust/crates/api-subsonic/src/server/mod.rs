mod albums;
mod annotations;
mod artists;
mod auth;
mod cached_lists;
mod counting;
mod folders;
mod library;
mod list_caches;
mod music_server;
mod pages;
mod play_queue;
mod playlist_cache;
mod playlists;
mod requests;
mod scanning;
mod sharing;
mod songs;
mod transport;
mod urls;

use std::time::Duration;

use feishin_api::{LatestCache, Result};
use feishin_core::model::{FolderId, ServerConfig};
use tokio::sync::OnceCell;

use crate::client::{Client, Params, RequestEncoding};
use crate::extensions::Extensions;
use crate::normalize::FolderListing;
use crate::types::{OpenSubsonicExtensions, Ping};
use list_caches::ListCaches;
use playlist_cache::PlaylistCache;

const FOLDER_TIME_TO_LIVE: Duration = Duration::from_secs(30);

pub struct SubsonicServer {
    config: ServerConfig,
    client: Client,
    extensions: OnceCell<Extensions>,
    list_caches: ListCaches,
    playlist_cache: PlaylistCache,
    latest_folder: LatestCache<FolderId, FolderListing>,
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
            list_caches: ListCaches::new(),
            playlist_cache: PlaylistCache::default(),
            latest_folder: LatestCache::new(FOLDER_TIME_TO_LIVE),
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
