use serde::Deserialize;

use super::lenient::{lenient_number, lenient_optional_id};
use super::song::SubsonicSong;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlayQueue {
    #[serde(deserialize_with = "lenient_optional_id")]
    pub current: Option<String>,
    #[serde(deserialize_with = "lenient_number")]
    pub current_index: Option<f64>,
    #[serde(deserialize_with = "lenient_number")]
    pub position: Option<f64>,
    pub entry: Option<Vec<SubsonicSong>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GetPlayQueue {
    pub play_queue: Option<PlayQueue>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GetPlayQueueByIndex {
    pub play_queue_by_index: Option<PlayQueue>,
}
