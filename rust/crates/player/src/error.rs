use std::ffi::c_int;

use crate::ffi;

#[derive(Debug, thiserror::Error)]
pub enum PlayerError {
    #[error("mpv error: {0}")]
    Mpv(String),
    #[error("the player has shut down")]
    Closed,
}

pub type Result<T> = std::result::Result<T, PlayerError>;

pub(crate) fn error_from_mpv_code(code: c_int) -> PlayerError {
    PlayerError::Mpv(ffi::error_string(code))
}

pub(crate) fn error_from_mpv(error: libmpv2::Error) -> PlayerError {
    match error {
        libmpv2::Error::Raw(code) => error_from_mpv_code(code),
        other => PlayerError::Mpv(other.to_string()),
    }
}

pub(crate) fn log_failure(result: Result<()>, action: &str) {
    if let Err(error) = result {
        tracing::warn!("{action} failed: {error}");
    }
}
