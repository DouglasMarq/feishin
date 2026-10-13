//! HTTP plumbing: native API requests with JWT refresh, Subsonic requests and URL building.

use std::future::Future;

use feishin_api::{ApiError, Result};
use reqwest::header::{CONTENT_TYPE, HeaderMap};
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use url::form_urlencoded;

use crate::NavidromeServer;
use crate::features::{EXT_FORM_POST, Extensions, Features, Ping, parse_version};

pub(crate) const ND_AUTH_HEADER: &str = "x-nd-authorization";
const TOTAL_COUNT_HEADER: &str = "x-total-count";
const API_VERSION: &str = "1.16.1";
const CLIENT_NAME: &str = "feishin";

pub(crate) fn network(e: reqwest::Error) -> ApiError {
    ApiError::Network(e.to_string())
}

pub(crate) fn decode(e: serde_json::Error) -> ApiError {
    ApiError::Decode(e.to_string())
}

pub(crate) fn no_session() -> ApiError {
    ApiError::Auth("Navidrome session token is missing or expired".into())
}

fn http_error(status: StatusCode, body: &[u8]) -> ApiError {
    match status {
        StatusCode::UNAUTHORIZED => ApiError::Auth(body_message(status, body)),
        StatusCode::NOT_FOUND => ApiError::NotFound,
        _ => ApiError::Server {
            code: i32::from(status.as_u16()),
            message: body_message(status, body),
        },
    }
}

fn body_message(status: StatusCode, body: &[u8]) -> String {
    let text = String::from_utf8_lossy(body).trim().to_owned();
    if text.is_empty() {
        status.to_string()
    } else {
        text
    }
}

fn subsonic_error(code: i64, message: String) -> ApiError {
    match code {
        40..=44 => ApiError::Auth(message),
        70 => ApiError::NotFound,
        _ => ApiError::Server {
            code: i32::try_from(code).unwrap_or(i32::MAX),
            message,
        },
    }
}

pub(crate) struct NativeResponse {
    body: Vec<u8>,
    pub total: Option<usize>,
}

impl NativeResponse {
    pub fn json<T: DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_slice(&self.body).map_err(decode)
    }
}

pub(crate) fn query_string(params: &[(&str, String)]) -> String {
    let mut s = form_urlencoded::Serializer::new(String::new());
    for (k, v) in params {
        s.append_pair(k, v);
    }
    s.finish()
}

impl NavidromeServer {
    pub(crate) fn token(&self) -> Option<String> {
        self.nd_token
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn set_token(&self, token: Option<String>) {
        *self.nd_token.write().unwrap_or_else(|e| e.into_inner()) = token;
    }

    fn capture_token(&self, headers: &HeaderMap) {
        let refreshed = headers
            .get(ND_AUTH_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.trim())
            .map(|v| v.strip_prefix("Bearer ").unwrap_or(v).trim())
            .filter(|v| !v.is_empty());
        if let Some(token) = refreshed {
            self.set_token(Some(token.to_owned()));
        }
    }

    pub(crate) async fn native(
        &self,
        method: Method,
        path: &str,
        params: &[(&str, String)],
        body: Option<&Value>,
    ) -> Result<NativeResponse> {
        let token = self.token().ok_or_else(no_session)?;
        let mut request = self
            .http
            .request(method, format!("{}/api/{path}", self.base_url))
            .query(params)
            .header(ND_AUTH_HEADER, format!("Bearer {token}"));
        if let Some(body) = body {
            request = request.json(body);
        }

        let response = request.send().await.map_err(network)?;
        self.capture_token(response.headers());
        let status = response.status();
        let total = response
            .headers()
            .get(TOTAL_COUNT_HEADER)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse().ok());
        let body = response.bytes().await.map_err(network)?.to_vec();

        if status == StatusCode::UNAUTHORIZED {
            self.set_token(None);
        }
        if !status.is_success() {
            return Err(http_error(status, &body));
        }
        Ok(NativeResponse { body, total })
    }

    pub(crate) async fn native_get<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<(T, Option<usize>)> {
        let response = self.native(Method::GET, path, params, None).await?;
        Ok((response.json()?, response.total))
    }

    /// Runs `native` when a session token exists, otherwise (or when it is rejected) `fallback`.
    pub(crate) async fn native_or<T>(
        &self,
        native: impl Future<Output = Result<T>>,
        fallback: impl Future<Output = Result<T>>,
    ) -> Result<T> {
        if self.token().is_some() {
            match native.await {
                Err(ApiError::Auth(_)) => {}
                other => return other,
            }
        }
        fallback.await
    }

    fn subsonic_query(&self, params: &[(&str, String)], json: bool) -> String {
        let mut common = vec![("v", API_VERSION.to_owned()), ("c", CLIENT_NAME.to_owned())];
        if json {
            common.push(("f", "json".to_owned()));
        }
        common.extend(params.iter().map(|(k, v)| (*k, v.clone())));
        let query = query_string(&common);
        if self.subsonic_auth.is_empty() {
            query
        } else {
            format!("{}&{query}", self.subsonic_auth)
        }
    }

    pub(crate) fn subsonic_url(
        &self,
        endpoint: &str,
        params: &[(&str, String)],
        json: bool,
    ) -> String {
        format!(
            "{}/rest/{endpoint}.view?{}",
            self.base_url,
            self.subsonic_query(params, json)
        )
    }

    pub(crate) async fn subsonic<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        params: &[(&str, String)],
    ) -> Result<T> {
        let response = self
            .http
            .get(self.subsonic_url(endpoint, params, true))
            .send()
            .await
            .map_err(network)?;
        Self::subsonic_body(response).await
    }

    /// Like [`Self::subsonic`] but sends the parameters as a form body when the server supports
    /// the OpenSubsonic `formPost` extension, which avoids URL length limits for long id lists.
    pub(crate) async fn subsonic_large<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        params: &[(&str, String)],
        features: &Features,
    ) -> Result<T> {
        if !features.has_extension(EXT_FORM_POST) {
            return self.subsonic(endpoint, params).await;
        }
        let response = self
            .http
            .post(format!("{}/rest/{endpoint}.view", self.base_url))
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(self.subsonic_query(params, true))
            .send()
            .await
            .map_err(network)?;
        Self::subsonic_body(response).await
    }

    async fn subsonic_body<T: DeserializeOwned>(response: reqwest::Response) -> Result<T> {
        let status = response.status();
        let body = response.bytes().await.map_err(network)?;
        if !status.is_success() {
            return Err(http_error(status, &body));
        }

        let mut envelope: Value = serde_json::from_slice(&body).map_err(decode)?;
        let inner = envelope
            .get_mut("subsonic-response")
            .map(Value::take)
            .ok_or_else(|| ApiError::Decode("missing subsonic-response".into()))?;

        if inner.get("status").and_then(Value::as_str) != Some("ok") {
            let error = inner.get("error");
            let code = error
                .and_then(|e| e.get("code"))
                .and_then(|c| c.as_i64().or_else(|| c.as_str()?.trim().parse().ok()))
                .unwrap_or(0);
            let message = error
                .and_then(|e| e.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("request failed")
                .to_owned();
            return Err(subsonic_error(code, message));
        }

        serde_json::from_value(inner).map_err(decode)
    }

    pub(crate) async fn features(&self) -> Features {
        self.features
            .get_or_try_init(|| self.detect_features())
            .await
            .cloned()
            .unwrap_or_default()
    }

    async fn detect_features(&self) -> Result<Features> {
        let ping: Ping = self.subsonic("ping", &[]).await?;
        let mut features = Features {
            version: ping.server_version.as_deref().and_then(parse_version),
            ..Features::default()
        };
        if ping.open_subsonic {
            let extensions: Extensions = self.subsonic("getOpenSubsonicExtensions", &[]).await?;
            features.extensions = extensions
                .open_subsonic_extensions
                .into_iter()
                .map(|e| (e.name, e.versions))
                .collect();
        }
        Ok(features)
    }
}
