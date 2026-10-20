use feishin_core::model::LibraryScan;
use serde::Deserialize;

use crate::lenient::{optional_u64, truthy};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct ScanStatus {
    #[serde(rename = "scanning", deserialize_with = "truthy")]
    pub is_scanning: bool,
    #[serde(deserialize_with = "optional_u64")]
    pub count: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct ScanStatusResponse {
    pub scan_status: Option<ScanStatus>,
}

impl From<ScanStatus> for LibraryScan {
    fn from(status: ScanStatus) -> Self {
        Self {
            is_scanning: status.is_scanning,
            scanned_items: status.count,
        }
    }
}
