use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::{ApiError, Result};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl fmt::Debug for Credential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let server_kind = match self {
            Self::Navidrome { .. } => "Navidrome",
            Self::Subsonic { .. } => "Subsonic",
            Self::Jellyfin { .. } => "Jellyfin",
        };
        formatter.debug_struct(server_kind).finish_non_exhaustive()
    }
}

impl Credential {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(|error| ApiError::Decode(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_a_credential_debug_output_hides_its_secrets() {
        let credentials = [
            Credential::Navidrome {
                subsonic: "u=feishin&s=salt&t=token".into(),
                nd_token: Some("session".into()),
            },
            Credential::Subsonic {
                subsonic: "u=feishin&p=password".into(),
            },
            Credential::Jellyfin {
                access_token: "access".into(),
                user_id: "user".into(),
            },
        ];
        let printed: Vec<String> = credentials
            .iter()
            .map(|credential| format!("{credential:?}"))
            .collect();
        assert_eq!(
            printed,
            ["Navidrome { .. }", "Subsonic { .. }", "Jellyfin { .. }"]
        );
    }
}
