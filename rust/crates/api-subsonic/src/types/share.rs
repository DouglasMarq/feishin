use serde::Deserialize;

use super::lenient::lenient_id;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct SubsonicShare {
    #[serde(deserialize_with = "lenient_id")]
    pub id: String,
    pub url: String,
    pub description: Option<String>,
    pub expires: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ShareList {
    pub share: Option<Vec<SubsonicShare>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Shares {
    pub shares: Option<ShareList>,
}
