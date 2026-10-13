use serde::Deserialize;

use crate::lenient::{optional_u32, optional_u64};
use crate::native::NativeSong;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct NativeQueue {
    #[serde(deserialize_with = "optional_u32")]
    pub current: Option<u32>,
    pub items: Option<Vec<NativeSong>>,
    #[serde(deserialize_with = "optional_u64")]
    pub position: Option<u64>,
}
