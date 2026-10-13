use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RadioStation {
    pub id: String,
    pub name: String,
    pub stream_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RadioStationDraft {
    pub name: String,
    pub stream_url: String,
    pub homepage_url: Option<String>,
}

impl RadioStationDraft {
    pub fn from_station(station: &RadioStation) -> Self {
        Self {
            name: station.name.clone(),
            stream_url: station.stream_url.clone(),
            homepage_url: station.homepage_url.clone(),
        }
    }
}
