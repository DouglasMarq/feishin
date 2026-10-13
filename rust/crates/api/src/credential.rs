use serde::{Deserialize, Serialize};

use crate::error::{ApiError, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Credential {
    Navidrome {
        subsonic: String,
        nd_token: Option<String>,
    },
    Subsonic {
        subsonic: String,
    },
    Jellyfin {
        access_token: String,
        user_id: String,
    },
}

impl Credential {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(|error| ApiError::Decode(error.to_string()))
    }
}
