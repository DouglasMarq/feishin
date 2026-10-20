use feishin_api::{ApiError, ErrorCause};
use reqwest::StatusCode;
use serde::Deserialize;

const MISSING_DATA_MESSAGE: &str = "data not found";
const NOT_AUTHORIZED_CODE: i64 = 50;

#[derive(Deserialize)]
struct NativeErrorBody {
    error: String,
}

fn reports_missing_data(body: &[u8]) -> bool {
    serde_json::from_slice::<NativeErrorBody>(body)
        .is_ok_and(|parsed| parsed.error == MISSING_DATA_MESSAGE)
}

pub(crate) fn network_error(error: reqwest::Error) -> ApiError {
    ApiError::Network(Box::new(error.without_url()))
}

pub(crate) fn invalid_address_error(reason: impl Into<ErrorCause>) -> ApiError {
    ApiError::InvalidAddress(reason.into())
}

pub(crate) fn decode_error(error: serde_json::Error) -> ApiError {
    ApiError::Decode(Box::new(error))
}

pub(crate) fn missing_session_error() -> ApiError {
    ApiError::Auth("Navidrome session token is missing or expired".into())
}

pub(crate) fn http_status_error(status: StatusCode, body: &[u8]) -> ApiError {
    match status {
        StatusCode::UNAUTHORIZED => ApiError::Auth(body_message(status, body)),
        StatusCode::FORBIDDEN => ApiError::NotAuthorized(body_message(status, body)),
        StatusCode::NOT_FOUND => ApiError::NotFound,
        _ if reports_missing_data(body) => ApiError::NotFound,
        _ => ApiError::Server {
            code: i32::from(status.as_u16()),
            message: body_message(status, body),
        },
    }
}

fn body_message(status: StatusCode, body: &[u8]) -> String {
    Some(String::from_utf8_lossy(body).trim().to_owned())
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| status.to_string())
}

pub(crate) fn subsonic_error(code: i64, message: String) -> ApiError {
    match code {
        40..=44 => ApiError::Auth(message),
        NOT_AUTHORIZED_CODE => ApiError::NotAuthorized(message),
        70 => ApiError::NotFound,
        _ => ApiError::Server {
            code: i32::try_from(code).unwrap_or(i32::MAX),
            message,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_servers_report_missing_items_as_server_errors_with_a_body() {
        let error = http_status_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            br#"{"error":"data not found"}"#,
        );
        assert!(matches!(error, ApiError::NotFound));
    }

    #[test]
    fn given_http_403_returns_not_authorized() {
        let error = http_status_error(StatusCode::FORBIDDEN, b"admin only");
        assert!(matches!(error, ApiError::NotAuthorized(message) if message == "admin only"));
    }

    #[test]
    fn other_server_errors_keep_their_message() {
        let error = http_status_error(StatusCode::INTERNAL_SERVER_ERROR, b"boom");
        assert!(matches!(error, ApiError::Server { code: 500, .. }));
    }
}
