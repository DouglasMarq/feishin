use feishin_core::model::RadioStation;
use serde::Deserialize;

use crate::lenient::{optional_string, string, truthy};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct SubsonicRadioStation {
    #[serde(deserialize_with = "string")]
    pub id: String,
    #[serde(deserialize_with = "string")]
    pub name: String,
    #[serde(deserialize_with = "string")]
    pub stream_url: String,
    #[serde(alias = "homepageUrl", deserialize_with = "optional_string")]
    pub home_page_url: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct RadioStationList {
    pub internet_radio_station: Vec<SubsonicRadioStation>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct InternetRadioStations {
    pub internet_radio_stations: Option<RadioStationList>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct SubsonicUser {
    #[serde(deserialize_with = "truthy")]
    pub admin_role: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct UserResponse {
    pub user: Option<SubsonicUser>,
}

pub(crate) fn normalize_radio_station(station: SubsonicRadioStation) -> RadioStation {
    RadioStation {
        id: station.id,
        name: station.name,
        stream_url: station.stream_url,
        homepage_url: station.home_page_url.filter(|url| !url.is_empty()),
    }
}
