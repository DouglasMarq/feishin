use feishin_api::{ApiError, Result};
use serde_json::Value;

const MAX_ERROR_BODY_CHARS: usize = 200;

pub struct SubsonicEnvelope {
    pub body: Value,
    pub tolerated_failure_message: Option<String>,
}

pub fn unwrap_subsonic_response(http_status: u16, text: &str) -> Result<SubsonicEnvelope> {
    let Some(body) = extract_subsonic_response(text) else {
        return Err(missing_envelope_error(http_status, text));
    };
    match body.get("status").and_then(Value::as_str) {
        Some("ok") => Ok(SubsonicEnvelope {
            body,
            tolerated_failure_message: None,
        }),
        _ => failed_envelope(body),
    }
}

fn extract_subsonic_response(text: &str) -> Option<Value> {
    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|mut document| document.get_mut("subsonic-response").map(Value::take))
        .filter(Value::is_object)
}

fn missing_envelope_error(http_status: u16, text: &str) -> ApiError {
    match http_status {
        401 | 403 => ApiError::Auth(format!("HTTP {http_status}")),
        404 => ApiError::NotFound,
        200..=299 => ApiError::Decode("missing subsonic-response".into()),
        _ => ApiError::Server {
            code: i32::from(http_status),
            message: text.chars().take(MAX_ERROR_BODY_CHARS).collect(),
        },
    }
}

fn failed_envelope(body: Value) -> Result<SubsonicEnvelope> {
    let error = body.get("error");
    let message = error
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("request failed")
        .to_owned();
    let code = error
        .and_then(|error| error.get("code"))
        .and_then(error_code);
    match code {
        Some(0) => Ok(SubsonicEnvelope {
            body,
            tolerated_failure_message: Some(message),
        }),
        Some(40..=44) => Err(ApiError::Auth(message)),
        Some(70) => Err(ApiError::NotFound),
        Some(code) => Err(ApiError::Server { code, message }),
        None => Err(ApiError::Server { code: 0, message }),
    }
}

fn error_code(value: &Value) -> Option<i32> {
    match value {
        Value::Number(number) => number.as_i64().and_then(|code| i32::try_from(code).ok()),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failed_with_code(code: &str) -> String {
        format!(
            r#"{{"subsonic-response":{{"status":"failed","version":"1.16.1","error":{{"code":{code},"message":"boom"}}}}}}"#
        )
    }

    #[test]
    fn failed_envelope_codes_map_to_api_errors() {
        assert!(matches!(
            unwrap_subsonic_response(200, &failed_with_code("40")),
            Err(ApiError::Auth(message)) if message == "boom"
        ));
        assert!(matches!(
            unwrap_subsonic_response(200, &failed_with_code("\"41\"")),
            Err(ApiError::Auth(_))
        ));
        assert!(matches!(
            unwrap_subsonic_response(200, &failed_with_code("70")),
            Err(ApiError::NotFound)
        ));
        assert!(matches!(
            unwrap_subsonic_response(200, &failed_with_code("10")),
            Err(ApiError::Server { code: 10, .. })
        ));
        assert!(matches!(
            unwrap_subsonic_response(200, &failed_with_code("0")),
            Ok(SubsonicEnvelope {
                tolerated_failure_message: Some(_),
                ..
            })
        ));
    }

    #[test]
    fn missing_envelope_maps_http_status() {
        assert!(matches!(
            unwrap_subsonic_response(401, "Unauthorized"),
            Err(ApiError::Auth(_))
        ));
        assert!(matches!(
            unwrap_subsonic_response(500, "oops"),
            Err(ApiError::Server { code: 500, .. })
        ));
        assert!(matches!(
            unwrap_subsonic_response(200, "<html>"),
            Err(ApiError::Decode(_))
        ));
    }
}
