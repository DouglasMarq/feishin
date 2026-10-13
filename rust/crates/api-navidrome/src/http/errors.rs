use feishin_api::ApiError;
use reqwest::StatusCode;

pub(crate) fn network_error(error: reqwest::Error) -> ApiError {
    ApiError::Network(error.to_string())
}

pub(crate) fn decode_error(error: serde_json::Error) -> ApiError {
    ApiError::Decode(error.to_string())
}

pub(crate) fn missing_session_error() -> ApiError {
    ApiError::Auth("Navidrome session token is missing or expired".into())
}

pub(crate) fn http_status_error(status: StatusCode, body: &[u8]) -> ApiError {
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
    Some(String::from_utf8_lossy(body).trim().to_owned())
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| status.to_string())
}

pub(crate) fn subsonic_error(code: i64, message: String) -> ApiError {
    match code {
        40..=44 => ApiError::Auth(message),
        70 => ApiError::NotFound,
        _ => ApiError::Server {
            code: i32::try_from(code).unwrap_or(i32::MAX),
            message,
        },
    }
}
