use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use reqwest::{Response, StatusCode};
use serde::de::{DeserializeOwned, IgnoredAny};

use crate::error::{DirectoryError, Result, network_error};
use crate::mirror::{ServerEntry, pick_mirror};
use crate::query::StationQuery;
use crate::station::{DirectoryStation, RawStation, playable_stations};

pub const PUBLIC_DIRECTORY_URL: &str = "https://all.api.radio-browser.info";
pub const USER_AGENT: &str = concat!(
    "Feishin/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/jeffvli/feishin)"
);

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

pub struct RadioBrowser {
    http: reqwest::Client,
    discovery_url: String,
    mirror: Mutex<Option<String>>,
}

impl RadioBrowser {
    pub fn public() -> Result<Self> {
        Self::with_discovery_url(PUBLIC_DIRECTORY_URL)
    }

    pub fn with_discovery_url(discovery_url: &str) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(network_error)?;
        Ok(Self {
            http,
            discovery_url: discovery_url.trim_end_matches('/').to_owned(),
            mirror: Mutex::new(None),
        })
    }

    pub async fn search(&self, query: &StationQuery) -> Result<Vec<DirectoryStation>> {
        let raw_stations: Vec<RawStation> = self
            .get_json("json/stations/search", &query.parameters())
            .await?;
        Ok(playable_stations(raw_stations))
    }

    pub async fn count_click(&self, station_uuid: &str) -> Result<()> {
        self.get_json::<IgnoredAny>(&format!("json/url/{station_uuid}"), &[])
            .await
            .map(drop)
    }

    async fn get_json<Decoded: DeserializeOwned>(
        &self,
        path: &str,
        parameters: &[(&str, String)],
    ) -> Result<Decoded> {
        match self.get_from_mirror(path, parameters).await {
            Err(error) if is_worth_retrying(&error) => {
                tracing::debug!("radio-browser request failed, trying again: {error}");
                self.forget_mirror();
                self.get_from_mirror(path, parameters).await
            }
            outcome => outcome,
        }
    }

    async fn get_from_mirror<Decoded: DeserializeOwned>(
        &self,
        path: &str,
        parameters: &[(&str, String)],
    ) -> Result<Decoded> {
        let base = self.mirror_url().await;
        let request = self.http.get(format!("{base}/{path}")).query(parameters);
        send_and_decode(request).await
    }

    async fn mirror_url(&self) -> String {
        let remembered = self.remembered_mirror().clone();
        if let Some(mirror) = remembered {
            return mirror;
        }
        let resolved = self.resolve_mirror().await;
        *self.remembered_mirror() = Some(resolved.clone());
        resolved
    }

    async fn resolve_mirror(&self) -> String {
        let request = self
            .http
            .get(format!("{}/json/servers", self.discovery_url));
        match send_and_decode::<Vec<ServerEntry>>(request).await {
            Ok(servers) => pick_mirror(&self.discovery_url, servers)
                .unwrap_or_else(|| self.discovery_url.clone()),
            Err(error) => {
                tracing::debug!("resolving a radio-browser mirror failed: {error}");
                self.discovery_url.clone()
            }
        }
    }

    fn remembered_mirror(&self) -> MutexGuard<'_, Option<String>> {
        self.mirror.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn forget_mirror(&self) {
        *self.remembered_mirror() = None;
    }
}

fn is_worth_retrying(error: &DirectoryError) -> bool {
    match error {
        DirectoryError::Network(_) => true,
        DirectoryError::Status(status) => *status >= 500,
        DirectoryError::Decode(_) => false,
    }
}

async fn send_and_decode<Decoded: DeserializeOwned>(
    request: reqwest::RequestBuilder,
) -> Result<Decoded> {
    let response = request.send().await.map_err(network_error)?;
    let body = successful_body(response).await?;
    serde_json::from_slice(&body).map_err(|error| DirectoryError::Decode(error.to_string()))
}

async fn successful_body(response: Response) -> Result<Vec<u8>> {
    let status = response.status();
    if status != StatusCode::OK {
        return Err(DirectoryError::Status(status.as_u16()));
    }
    Ok(response.bytes().await.map_err(network_error)?.to_vec())
}
