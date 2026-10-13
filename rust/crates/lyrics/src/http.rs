use serde::de::DeserializeOwned;

use crate::{LyricsError, Result};

async fn send(request: reqwest::RequestBuilder) -> Result<Option<reqwest::Response>> {
    let response = request.send().await.map_err(network_error)?;
    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !status.is_success() {
        return Err(LyricsError::Network(format!("HTTP {status}")));
    }
    Ok(Some(response))
}

pub(crate) async fn send_json<T: DeserializeOwned>(
    request: reqwest::RequestBuilder,
) -> Result<Option<T>> {
    let Some(response) = send(request).await? else {
        return Ok(None);
    };
    let body = response.bytes().await.map_err(network_error)?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|error| LyricsError::Decode(error.to_string()))
}

pub(crate) async fn send_text(request: reqwest::RequestBuilder) -> Result<Option<String>> {
    let Some(response) = send(request).await? else {
        return Ok(None);
    };
    response.text().await.map(Some).map_err(network_error)
}

fn network_error(error: reqwest::Error) -> LyricsError {
    LyricsError::Network(error.to_string())
}
