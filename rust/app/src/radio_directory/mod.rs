mod demo;

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use feishin_radio_browser::{DirectoryStation, RadioBrowser, StationQuery};

pub use demo::DemoDirectory;

#[async_trait]
pub trait StationDirectory: Send + Sync {
    async fn search(&self, query: &StationQuery) -> Result<Vec<DirectoryStation>>;
    async fn count_click(&self, station_uuid: &str) -> Result<()>;
}

#[async_trait]
impl StationDirectory for RadioBrowser {
    async fn search(&self, query: &StationQuery) -> Result<Vec<DirectoryStation>> {
        Ok(RadioBrowser::search(self, query).await?)
    }

    async fn count_click(&self, station_uuid: &str) -> Result<()> {
        Ok(RadioBrowser::count_click(self, station_uuid).await?)
    }
}

pub struct UnavailableDirectory(String);

#[async_trait]
impl StationDirectory for UnavailableDirectory {
    async fn search(&self, _query: &StationQuery) -> Result<Vec<DirectoryStation>> {
        Err(anyhow::anyhow!("{}", self.0))
    }

    async fn count_click(&self, _station_uuid: &str) -> Result<()> {
        Ok(())
    }
}

pub fn public_directory() -> Arc<dyn StationDirectory> {
    match RadioBrowser::public() {
        Ok(directory) => Arc::new(directory),
        Err(error) => {
            tracing::warn!("the radio directory is unavailable: {error}");
            Arc::new(UnavailableDirectory(error.to_string()))
        }
    }
}
