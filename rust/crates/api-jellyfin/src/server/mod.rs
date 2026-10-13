mod albums;
mod artists;
mod library;
mod music_server;
mod playback;
mod playlist_items;
mod playlists;
mod scanning;
mod search;
mod session;
mod songs;
mod stream_urls;

use feishin_api::Result;
use feishin_core::model::ServerConfig;
use reqwest::{Method, RequestBuilder, header::AUTHORIZATION};
use serde::de::DeserializeOwned;

use crate::authorization::auth_header;
use crate::query_params::QueryParams;
use crate::transport::{decode, send};

const MAX_IDS_PER_QUERY_STRING: usize = 50;

pub struct JellyfinServer {
    config: ServerConfig,
    base_url: String,
    access_token: String,
    user_id: String,
    http: reqwest::Client,
}

impl JellyfinServer {
    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        self.http
            .request(method, format!("{}/{path}", self.base_url))
            .header(
                AUTHORIZATION,
                auth_header(&self.config.id, Some(&self.access_token)),
            )
    }

    async fn execute(&self, request: RequestBuilder) -> Result<()> {
        send(request).await?;
        Ok(())
    }

    async fn get<Body: DeserializeOwned>(&self, path: &str, params: &QueryParams) -> Result<Body> {
        let response = send(self.request(Method::GET, path).query(params.pairs())).await?;
        decode(response).await
    }

    fn items_path(&self) -> String {
        format!("users/{}/items", self.user_id)
    }

    fn item_path(&self, id: &str) -> String {
        format!("users/{}/items/{id}", self.user_id)
    }

    fn server_id(&self) -> &str {
        &self.config.id
    }
}
