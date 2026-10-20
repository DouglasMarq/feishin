use std::time::Duration;

use serde::de::DeserializeOwned;

use crate::{LyricsError, Result};

pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

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
    let body = capped_body(response).await?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|error| LyricsError::Decode(error.to_string()))
}

pub(crate) async fn send_text(request: reqwest::RequestBuilder) -> Result<Option<String>> {
    let Some(response) = send(request).await? else {
        return Ok(None);
    };
    let body = capped_body(response).await?;
    Ok(Some(String::from_utf8_lossy(&body).into_owned()))
}

async fn capped_body(mut response: reqwest::Response) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(network_error)? {
        if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(LyricsError::Decode(format!(
                "the response is larger than {MAX_RESPONSE_BYTES} bytes"
            )));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn network_error(error: reqwest::Error) -> LyricsError {
    LyricsError::Network(error.to_string())
}
