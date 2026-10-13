use std::ffi::c_int;

use crate::ffi;

#[derive(Debug, thiserror::Error)]
pub enum PlayerError {
    #[error("mpv error: {0}")]
    Mpv(String),
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
