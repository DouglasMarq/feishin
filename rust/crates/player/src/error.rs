use std::ffi::{NulError, c_int};

use crate::ffi;

#[derive(Debug, thiserror::Error)]
pub enum PlayerError {
    #[error("mpv error: {}", ffi::error_string(*.0))]
    Mpv(c_int),
    #[error("mpv error: {0}")]
    Library(#[source] libmpv2::Error),
    #[error("mpv error: {0}")]
    InvalidArgument(#[from] NulError),
    #[error("failed to spawn the {name} thread: {source}")]
    Thread {
        name: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("mpv error: {0}")]
    UnexpectedReply(&'static str),
    #[error("the player has shut down")]
    Closed,
}

pub type Result<T> = std::result::Result<T, PlayerError>;

pub(crate) fn error_from_mpv(error: libmpv2::Error) -> PlayerError {
    match error {
        libmpv2::Error::Raw(code) => PlayerError::Mpv(code),
        other => PlayerError::Library(other),
    }
}

pub(crate) fn log_failure(result: Result<()>, action: &str) {
    if let Err(error) = result {
        tracing::warn!("{action} failed: {error}");
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::ffi::CString;

    use super::*;

    #[test]
    fn given_an_argument_with_a_nul_byte_the_source_is_the_nul_error() {
        let nul_error = CString::new("a\0b").unwrap_err();
        let error = PlayerError::from(nul_error);
        assert!(error.source().unwrap().downcast_ref::<NulError>().is_some());
    }

    #[test]
    fn given_a_thread_that_cannot_start_the_source_is_the_io_error() {
        let error = PlayerError::Thread {
            name: "crossfade",
            source: std::io::Error::from(std::io::ErrorKind::OutOfMemory),
        };
        assert!(
            error
                .source()
                .unwrap()
                .downcast_ref::<std::io::Error>()
                .is_some()
        );
        assert!(
            error
                .to_string()
                .starts_with("failed to spawn the crossfade thread: ")
        );
    }

    #[test]
    fn given_a_library_error_the_source_is_the_libmpv_error() {
        let error = error_from_mpv(libmpv2::Error::Null);
        assert!(
            error
                .source()
                .unwrap()
                .downcast_ref::<libmpv2::Error>()
                .is_some()
        );
    }
}
