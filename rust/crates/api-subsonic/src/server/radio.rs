use feishin_api::Result;
use feishin_core::model::{RadioStation, RadioStationDraft};

use super::SubsonicServer;
use crate::client::Params;
use crate::normalize;
use crate::types::{InternetRadioStations, UserResponse};

fn station_params(station: &RadioStationDraft) -> Params {
    Params::new()
        .add("name", &station.name)
        .add("streamUrl", &station.stream_url)
        .add_optional("homepageUrl", station.homepage_url.as_deref())
}

impl SubsonicServer {
    pub(super) async fn fetch_radio_stations(&self) -> Result<Vec<RadioStation>> {
        let response: InternetRadioStations = self
            .fetch("getInternetRadioStations.view", Params::new())
            .await?;
        Ok(response
            .internet_radio_stations
            .and_then(|container| container.internet_radio_station)
            .unwrap_or_default()
            .into_iter()
            .map(normalize::radio_station)
            .collect())
    }

    pub(super) async fn is_admin(&self) -> Result<bool> {
        let params = Params::new().add("username", &self.config.username);
        let response: UserResponse = self.fetch("getUser.view", params).await?;
        Ok(response.user.is_some_and(|user| user.admin_role))
    }

    pub(super) async fn add_radio_station(&self, station: &RadioStationDraft) -> Result<()> {
        self.send("createInternetRadioStation.view", station_params(station))
            .await
    }

    pub(super) async fn change_radio_station(
        &self,
        id: &str,
        station: &RadioStationDraft,
    ) -> Result<()> {
        let params = station_params(station).add("id", id);
        self.send("updateInternetRadioStation.view", params).await
    }

    pub(super) async fn remove_radio_station(&self, id: &str) -> Result<()> {
        let params = Params::new().add("id", id);
        self.send("deleteInternetRadioStation.view", params).await
    }
}
