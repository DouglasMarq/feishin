use feishin_core::model::Share;
use serde::Deserialize;

use crate::conversions::non_empty;
use crate::lenient::{optional_string, string};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct SubsonicShare {
    #[serde(deserialize_with = "string")]
    pub id: String,
    #[serde(deserialize_with = "string")]
    pub url: String,
    #[serde(deserialize_with = "optional_string")]
    pub description: Option<String>,
    #[serde(deserialize_with = "optional_string")]
    pub expires: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct ShareList {
    pub share: Vec<SubsonicShare>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct SharesResponse {
    pub shares: Option<ShareList>,
}

pub(crate) fn normalize_share(share: SubsonicShare) -> Share {
    Share {
        id: share.id,
        url: share.url,
        description: non_empty(share.description),
        expires_at: non_empty(share.expires),
    }
}
