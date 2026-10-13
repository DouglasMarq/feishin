#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MusicError {
    #[error("YouTube Music could not be reached: {0}")]
    Network(String),
    #[error("YouTube Music answered with HTTP {0}")]
    Status(u16),
    #[error("YouTube Music sent an unexpected answer: {0}")]
    Decode(String),
    #[error("{0} is not a YouTube Music page")]
    InvalidId(String),
    #[error("signing in to YouTube Music failed: {0}")]
    SignIn(String),
}

impl MusicError {
    pub fn is_rejected_sign_in(&self) -> bool {
        matches!(self, Self::Status(401 | 403))
    }
}
