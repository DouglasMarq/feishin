use std::future::Future;

use feishin_api::{ApiError, Result};
use reqwest::header::HeaderMap;
use reqwest::{Method, RequestBuilder, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::NavidromeServer;
use crate::http::errors::{decode_error, http_status_error, missing_session_error, network_error};
use crate::http::session_token::SESSION_TOKEN_HEADER;

const TOTAL_COUNT_HEADER: &str = "x-total-count";

pub(crate) struct NativeResponse {
    body: Vec<u8>,
    pub total: Option<usize>,
}

impl NativeResponse {
    pub fn json<Decoded: DeserializeOwned>(&self) -> Result<Decoded> {
        serde_json::from_slice(&self.body).map_err(decode_error)
    }
}

impl NavidromeServer {
    pub(crate) async fn native(
        &self,
        method: Method,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<NativeResponse> {
        let request = self.native_request(method, path, params)?;
        self.send_native(request).await
    }

    pub(crate) async fn native_with_body<Body: Serialize + ?Sized>(
        &self,
        method: Method,
        path: &str,
        body: &Body,
    ) -> Result<NativeResponse> {
        let request = self.native_request(method, path, &[])?.json(body);
        self.send_native(request).await
    }

    pub(crate) async fn native_get<Decoded: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<(Decoded, Option<usize>)> {
        let response = self.native(Method::GET, path, params).await?;
        Ok((response.json()?, response.total))
    }

    pub(crate) async fn native_or_subsonic<Output>(
        &self,
        native: impl Future<Output = Result<Output>>,
        subsonic: impl Future<Output = Result<Output>>,
    ) -> Result<Output> {
        if !self.has_session_token() {
            return subsonic.await;
        }
        match native.await {
            Err(ApiError::Auth(_)) => subsonic.await,
            outcome => outcome,
        }
    }

    fn native_request(
        &self,
        method: Method,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<RequestBuilder> {
        let token = self
            .current_session_token()
            .ok_or_else(missing_session_error)?;
        Ok(self
            .http_client
            .request(method, format!("{}/api/{path}", self.base_url))
            .query(params)
            .header(SESSION_TOKEN_HEADER, format!("Bearer {token}")))
    }

    async fn send_native(&self, request: RequestBuilder) -> Result<NativeResponse> {
        let response = request.send().await.map_err(network_error)?;
        self.capture_refreshed_token(response.headers());
        let status = response.status();
        let total = total_count(response.headers());
        let body = response.bytes().await.map_err(network_error)?.to_vec();

        if status == StatusCode::UNAUTHORIZED {
            self.replace_session_token(None);
        }
        if !status.is_success() {
            return Err(http_status_error(status, &body));
        }
        Ok(NativeResponse { body, total })
    }
}

fn total_count(headers: &HeaderMap) -> Option<usize> {
    headers
        .get(TOTAL_COUNT_HEADER)
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.trim().parse().ok())
}
