use serde::Deserialize;

use super::lenient::lenient_id;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SubsonicRadioStation {
    #[serde(deserialize_with = "lenient_id")]
    pub id: String,
    pub name: String,
    pub stream_url: String,
    #[serde(alias = "homepageUrl")]
    pub home_page_url: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RadioStationContainer {
    pub internet_radio_station: Option<Vec<SubsonicRadioStation>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct InternetRadioStations {
    pub internet_radio_stations: Option<RadioStationContainer>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SubsonicUser {
    pub admin_role: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct UserResponse {
    pub user: Option<SubsonicUser>,
}
