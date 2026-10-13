use std::time::Duration;

use feishin_api::{ApiError, Result};
use md5::{Digest, Md5};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use rand::RngExt;
use serde::de::DeserializeOwned;
use serde_json::Value;

pub const API_VERSION: &str = "1.13.0";
pub const CLIENT_NAME: &str = "Feishin";

const URI_COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

/// Percent-encodes like JavaScript's `encodeURIComponent`, matching credentials stored by Electron Feishin.
pub fn encode(value: &str) -> String {
    utf8_percent_encode(value, URI_COMPONENT).to_string()
}

pub fn random_salt() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    (0..12)
        .map(|_| char::from(CHARSET[rng.random_range(0..CHARSET.len())]))
        .collect()
}

pub fn md5_hex(input: &str) -> String {
    Md5::digest(input.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn token_credential(username: &str, password: &str) -> String {
    let salt = random_salt();
    let token = md5_hex(&format!("{password}{salt}"));
    format!(
        "u={}&s={}&t={}",
        encode(username),
        encode(&salt),
        encode(&token)
    )
}

pub fn password_credential(username: &str, password: &str) -> String {
    format!("u={}&p={}", encode(username), encode(password))
}

/// Accepts `u=..&s=..&t=..` or `u=..&p=..`.
pub fn validate_credential(credential: &str) -> Result<()> {
    let keys: Vec<&str> = credential
        .split('&')
        .filter_map(|pair| pair.split_once('=').map(|(k, _)| k))
        .collect();
    let has = |k: &str| keys.contains(&k);
    if has("u") && ((has("s") && has("t")) || has("p")) {
        Ok(())
    } else {
        Err(ApiError::Auth("malformed subsonic credential".into()))
    }
}

#[derive(Debug, Default, Clone)]
pub struct Params(Vec<(&'static str, String)>);

impl Params {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(mut self, key: &'static str, value: impl ToString) -> Self {
        self.0.push((key, value.to_string()));
        self
    }

    pub fn opt<V: ToString>(self, key: &'static str, value: Option<V>) -> Self {
        match value {
            Some(v) => self.add(key, v),
            None => self,
        }
    }

    pub fn many(mut self, key: &'static str, values: &[String]) -> Self {
        self.0.extend(values.iter().map(|v| (key, v.clone())));
        self
    }

    fn write(&self, out: &mut String) {
        for (key, value) in &self.0 {
            out.push('&');
            out.push_str(key);
            out.push('=');
            out.push_str(&encode(value));
        }
    }
}

pub struct Payload {
    endpoint: String,
    body: Value,
    soft_error: Option<String>,
    pub total_count: Option<usize>,
}

impl Payload {
    pub fn decode<T: DeserializeOwned>(self) -> Result<T> {
        let Payload {
            endpoint,
            body,
            soft_error,
            ..
        } = self;
        serde_json::from_value(body).map_err(|e| match soft_error {
            Some(message) => ApiError::Server { code: 0, message },
            None => ApiError::Decode(format!("{endpoint}: {e}")),
        })
    }
}

pub struct Client {
    http: reqwest::Client,
    base: String,
    auth: String,
}

impl Client {
    pub fn new(base: &str, auth: String) -> Result<Self> {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| ApiError::Network(e.to_string()))?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_owned(),
            auth,
        })
    }

    pub fn auth(&self) -> &str {
        &self.auth
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    fn query(&self, params: &Params) -> String {
        let mut out = format!("{}&v={API_VERSION}&c={CLIENT_NAME}&f=json", self.auth);
        params.write(&mut out);
        out
    }

    pub async fn call(&self, endpoint: &str, params: &Params, form_post: bool) -> Result<Payload> {
        let url = format!("{}/rest/{endpoint}", self.base);
        let query = self.query(params);
        let request = if form_post {
            self.http
                .post(url)
                .header(
                    reqwest::header::CONTENT_TYPE,
                    "application/x-www-form-urlencoded",
                )
                .body(query)
        } else {
            self.http.get(format!("{url}?{query}"))
        };
        let response = request
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;
        let status = response.status().as_u16();
        let total_count = response
            .headers()
            .get("x-total-count")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse().ok());
        let text = response
            .text()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;
        let (body, soft_error) = parse_envelope(status, &text)?;
        Ok(Payload {
            endpoint: endpoint.to_owned(),
            body,
            soft_error,
            total_count,
        })
    }
}

fn as_code(value: &Value) -> Option<i32> {
    match value {
        Value::Number(n) => n.as_i64().and_then(|n| i32::try_from(n).ok()),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// Unwraps `subsonic-response`. A `failed` status with code 0 is not treated as an error (the TS client
/// does the same, Navidrome uses it for unlinked Last.fm/Spotify); decoding failures then surface it.
fn parse_envelope(status: u16, text: &str) -> Result<(Value, Option<String>)> {
    let inner = serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|mut v| v.get_mut("subsonic-response").map(Value::take))
        .filter(Value::is_object);

    let Some(inner) = inner else {
        return Err(match status {
            401 | 403 => ApiError::Auth(format!("HTTP {status}")),
            404 => ApiError::NotFound,
            200..=299 => ApiError::Decode("missing subsonic-response".into()),
            _ => ApiError::Server {
                code: i32::from(status),
                message: text.chars().take(200).collect(),
            },
        });
    };

    if inner.get("status").and_then(Value::as_str) == Some("ok") {
        return Ok((inner, None));
    }

    let error = inner.get("error");
    let message = error
        .and_then(|e| e.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("request failed")
        .to_owned();
    match error.and_then(|e| e.get("code")).and_then(as_code) {
        Some(0) => Ok((inner, Some(message))),
        Some(40..=44) => Err(ApiError::Auth(message)),
        Some(70) => Err(ApiError::NotFound),
        Some(code) => Err(ApiError::Server { code, message }),
        None => Err(ApiError::Server { code: 0, message }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_like_encode_uri_component() {
        assert_eq!(encode("a b&c=d/é!*'()~"), "a%20b%26c%3Dd%2F%C3%A9!*'()~");
    }

    #[test]
    fn md5_matches_reference() {
        assert_eq!(md5_hex("sesamec19b2d"), "26719a1196d2a940705a59634eb18eab");
    }

    #[test]
    fn salt_is_alphanumeric() {
        let salt = random_salt();
        assert_eq!(salt.len(), 12);
        assert!(salt.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn validates_credentials() {
        assert!(validate_credential("u=a&s=b&t=c").is_ok());
        assert!(validate_credential("u=a&p=b").is_ok());
        assert!(validate_credential("u=a&s=b").is_err());
        assert!(validate_credential("").is_err());
    }

    #[test]
    fn envelope_codes() {
        let failed = |code: &str| {
            format!(
                r#"{{"subsonic-response":{{"status":"failed","version":"1.16.1","error":{{"code":{code},"message":"boom"}}}}}}"#
            )
        };
        assert!(matches!(
            parse_envelope(200, &failed("40")),
            Err(ApiError::Auth(m)) if m == "boom"
        ));
        assert!(matches!(
            parse_envelope(200, &failed("\"41\"")),
            Err(ApiError::Auth(_))
        ));
        assert!(matches!(
            parse_envelope(200, &failed("70")),
            Err(ApiError::NotFound)
        ));
        assert!(matches!(
            parse_envelope(200, &failed("10")),
            Err(ApiError::Server { code: 10, .. })
        ));
        assert!(matches!(
            parse_envelope(200, &failed("0")),
            Ok((_, Some(_)))
        ));
        assert!(matches!(
            parse_envelope(401, "Unauthorized"),
            Err(ApiError::Auth(_))
        ));
        assert!(matches!(
            parse_envelope(500, "oops"),
            Err(ApiError::Server { code: 500, .. })
        ));
        assert!(matches!(
            parse_envelope(200, "<html>"),
            Err(ApiError::Decode(_))
        ));
    }
}
