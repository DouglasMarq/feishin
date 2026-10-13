use serde::Deserialize;

use crate::lenient::{optional_string, optional_u32, optional_u64};
use crate::subsonic::song::SubsonicSong;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct PlayQueueBody {
    #[serde(deserialize_with = "optional_string")]
    pub current: Option<String>,
    #[serde(rename = "currentIndex", deserialize_with = "optional_u32")]
    pub current_index: Option<u32>,
    pub entry: Option<Vec<SubsonicSong>>,
    #[serde(deserialize_with = "optional_u64")]
    pub position: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct PlayQueue {
    pub play_queue: Option<PlayQueueBody>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct PlayQueueByIndex {
    pub play_queue_by_index: Option<PlayQueueBody>,
}
