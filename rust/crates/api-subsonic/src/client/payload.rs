use feishin_api::{ApiError, Result};
use serde::de::DeserializeOwned;

use super::envelope::SubsonicEnvelope;

pub struct Payload {
    endpoint: String,
    envelope: SubsonicEnvelope,
    pub total_count: Option<usize>,
}

impl Payload {
    pub(super) fn new(
        endpoint: &str,
        envelope: SubsonicEnvelope,
        total_count: Option<usize>,
    ) -> Self {
        Self {
            endpoint: endpoint.to_owned(),
            envelope,
            total_count,
        }
    }

    pub fn decode<T: DeserializeOwned>(self) -> Result<T> {
        let Self {
            endpoint,
            envelope:
                SubsonicEnvelope {
                    body,
                    tolerated_failure_message,
                },
            ..
        } = self;
        serde_json::from_value(body).map_err(|error| match tolerated_failure_message {
            Some(message) => ApiError::Server { code: 0, message },
            None => ApiError::Decode(format!("{endpoint}: {error}")),
        })
    }
}
