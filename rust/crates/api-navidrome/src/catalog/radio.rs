use feishin_api::Result;
use feishin_core::model::{RadioStation, RadioStationDraft};
use serde::de::IgnoredAny;

use crate::NavidromeServer;
use crate::subsonic::{InternetRadioStations, UserResponse, normalize_radio_station};

fn station_params(station: &RadioStationDraft) -> Vec<(&'static str, String)> {
    let mut params = vec![
        ("name", station.name.clone()),
        ("streamUrl", station.stream_url.clone()),
    ];
    params.extend(
        station
            .homepage_url
            .clone()
            .map(|homepage_url| ("homepageUrl", homepage_url)),
    );
    params
}

impl NavidromeServer {
    pub(crate) async fn fetch_radio_stations(&self) -> Result<Vec<RadioStation>> {
        let response = self
            .subsonic::<InternetRadioStations>("getInternetRadioStations", &[])
            .await?;
        Ok(response
            .internet_radio_stations
            .unwrap_or_default()
            .internet_radio_station
            .into_iter()
            .map(normalize_radio_station)
            .collect())
    }

    pub(crate) async fn is_admin(&self) -> Result<bool> {
        let params = [("username", self.config.username.clone())];
        let response = self.subsonic::<UserResponse>("getUser", &params).await?;
        Ok(response.user.is_some_and(|user| user.admin_role))
    }

    pub(crate) async fn add_radio_station(&self, station: &RadioStationDraft) -> Result<()> {
        self.subsonic::<IgnoredAny>("createInternetRadioStation", &station_params(station))
            .await
            .map(drop)
    }

    pub(crate) async fn change_radio_station(
        &self,
        id: &str,
        station: &RadioStationDraft,
    ) -> Result<()> {
        let mut params = station_params(station);
        params.push(("id", id.to_owned()));
        self.subsonic::<IgnoredAny>("updateInternetRadioStation", &params)
            .await
            .map(drop)
    }

    pub(crate) async fn remove_radio_station(&self, id: &str) -> Result<()> {
        self.subsonic::<IgnoredAny>("deleteInternetRadioStation", &[("id", id.to_owned())])
            .await
            .map(drop)
    }
}
