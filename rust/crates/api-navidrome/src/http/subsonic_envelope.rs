use feishin_api::{ApiError, Result};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::http::errors::{decode_error, subsonic_error};

const ENVELOPE_KEY: &str = "subsonic-response";
const OK_STATUS: &str = "ok";
const FALLBACK_ERROR_MESSAGE: &str = "request failed";

pub(crate) fn decode_subsonic_envelope<Decoded: DeserializeOwned>(body: &[u8]) -> Result<Decoded> {
    let mut envelope: Value = serde_json::from_slice(body).map_err(decode_error)?;
    let inner = envelope
        .get_mut(ENVELOPE_KEY)
        .map(Value::take)
        .ok_or_else(|| ApiError::Decode("missing subsonic-response".into()))?;
    if !is_ok_status(&inner) {
        return Err(failure_error(&inner));
    }
    serde_json::from_value(inner).map_err(decode_error)
}

fn is_ok_status(inner: &Value) -> bool {
    inner.get("status").and_then(Value::as_str) == Some(OK_STATUS)
}

fn failure_error(inner: &Value) -> ApiError {
    let error = inner.get("error");
    let code = error
        .and_then(|error| error.get("code"))
        .and_then(error_code)
        .unwrap_or(0);
    let message = error
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
        .unwrap_or(FALLBACK_ERROR_MESSAGE)
        .to_owned();
    subsonic_error(code, message)
}

fn error_code(code: &Value) -> Option<i64> {
    code.as_i64().or_else(|| code.as_str()?.trim().parse().ok())
}
