mod catalog;
mod connection;
mod conversions;
mod features;
mod http;
mod lenient;
mod music_server;
mod native;
mod query;
mod subsonic;

use std::sync::RwLock;

use feishin_core::model::ServerConfig;
use tokio::sync::OnceCell;

use crate::catalog::FallbackLists;
use crate::features::Features;

pub struct NavidromeServer {
    config: ServerConfig,
    base_url: String,
    http_client: reqwest::Client,
    subsonic_auth: String,
    session_token: RwLock<Option<String>>,
    detected_features: OnceCell<Features>,
    fallback_lists: FallbackLists,
}
