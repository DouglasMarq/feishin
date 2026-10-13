use feishin_api::{ApiError, Result};
use feishin_core::model::ServerConfig;
use reqwest::{RequestBuilder, Response, StatusCode};
use serde::de::DeserializeOwned;

pub fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .build()
        .map_err(|error| ApiError::Network(error.to_string()))
}

pub fn base_url(config: &ServerConfig) -> String {
    config.url.trim_end_matches('/').to_owned()
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
