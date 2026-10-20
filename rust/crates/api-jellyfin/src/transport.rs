use std::time::Duration;

use feishin_api::{ApiError, Result};
use feishin_core::model::ServerConfig;
use reqwest::{RequestBuilder, Response, StatusCode, Url};
use serde::de::DeserializeOwned;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

pub fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|error| ApiError::Network(error.to_string()))
}

pub fn base_url(config: &ServerConfig) -> Result<Url> {
    let url = Url::parse(&config.url)
        .map_err(|error| ApiError::Network(format!("invalid server address: {error}")))?;
    match url.cannot_be_a_base() {
        true => Err(ApiError::Network(format!(
            "invalid server address: {} cannot hold a path",
            url.scheme()
        ))),
        false => Ok(url),
    }
}

pub fn endpoint(base_url: &Url, path: &[&str]) -> Url {
    let mut url = base_url.clone();
    if let Ok(mut segments) = url.path_segments_mut() {
        segments.pop_if_empty().extend(path);
    }
    url
}

pub async fn send(request: RequestBuilder) -> Result<Response> {
    let response = request
        .send()
        .await
        .map_err(|error| ApiError::Network(error.to_string()))?;
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let message = response.text().await.unwrap_or_default();
    Err(error_for_status(status, message))
}

fn error_for_status(status: StatusCode, message: String) -> ApiError {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            ApiError::Auth(message_or_status(status, message))
        }
        StatusCode::NOT_FOUND => ApiError::NotFound,
        _ => ApiError::Server {
            code: i32::from(status.as_u16()),
            message,
        },
    }
}

fn message_or_status(status: StatusCode, message: String) -> String {
    Some(message)
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| status.to_string())
}

pub async fn decode<Body: DeserializeOwned>(response: Response) -> Result<Body> {
    let bytes = response
        .bytes()
        .await
        .map_err(|error| ApiError::Network(error.to_string()))?;
    serde_json::from_slice(&bytes).map_err(|error| ApiError::Decode(error.to_string()))
}
