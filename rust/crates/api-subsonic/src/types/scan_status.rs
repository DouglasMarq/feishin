use serde::Deserialize;

use super::lenient::lenient_number;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ScanStatus {
    pub scanning: bool,
    #[serde(deserialize_with = "lenient_number")]
    pub count: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScanStatusResponse {
    pub scan_status: Option<ScanStatus>,
}
