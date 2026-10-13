use serde::Deserialize;

use super::null_as_default;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ScheduledTask {
    #[serde(deserialize_with = "null_as_default")]
    pub key: String,
    #[serde(deserialize_with = "null_as_default")]
    pub state: String,
}
