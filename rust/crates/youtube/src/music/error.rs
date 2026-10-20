use std::sync::Arc;

use reqwest::StatusCode;

use crate::YouTubeError;

pub type ErrorCause = Arc<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum MusicError {
    #[error("YouTube Music could not be reached: {0}")]
    Network(#[source] ErrorCause),
    #[error("YouTube Music answered with HTTP {0}")]
    Status(u16),
    #[error("YouTube Music sent an unexpected answer: {0}")]
    Decode(String),
    #[error("YouTube Music sent an unexpected answer: {0}")]
    Malformed(#[source] Arc<serde_json::Error>),
    #[error("{0} is not a YouTube Music page")]
    InvalidId(String),
    #[error("signing in to YouTube Music failed: {0}")]
    SignIn(String),
    #[error("signing in to YouTube Music failed: {0}")]
    CookieExport(#[source] YouTubeError),
    #[error("signing in to YouTube Music failed: {context}: {source}")]
    CookieFile {
        context: &'static str,
        #[source]
        source: Arc<std::io::Error>,
    },
}

impl MusicError {
    pub fn is_rejected_sign_in(&self) -> bool {
        let Self::Status(status) = self else {
            return false;
        };
        matches!(
            StatusCode::from_u16(*status),
            Ok(StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN)
        )
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;

    #[test]
    fn given_a_cookie_file_failure_the_source_is_the_io_error() {
        let error = MusicError::CookieFile {
            context: "could not create the cookie file",
            source: Arc::new(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
        };
        let source = error.source().unwrap();
        assert!(source.downcast_ref::<Arc<std::io::Error>>().is_some());
    }

    #[test]
    fn given_a_failed_cookie_export_the_source_is_the_yt_dlp_error() {
        let error = MusicError::CookieExport(YouTubeError::NotInstalled);
        let source = error.source().unwrap();
        assert!(matches!(
            source.downcast_ref::<YouTubeError>(),
            Some(YouTubeError::NotInstalled)
        ));
    }
}
