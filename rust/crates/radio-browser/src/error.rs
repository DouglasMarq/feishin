#[derive(Debug, thiserror::Error)]
pub enum DirectoryError {
    #[error("radio directory unreachable: {0}")]
    Network(String),
    #[error("radio directory answered with HTTP {0}")]
    Status(u16),
    #[error("unexpected radio directory response: {0}")]
    Decode(String),
}

pub type Result<T> = std::result::Result<T, DirectoryError>;

pub(crate) fn network_error(error: reqwest::Error) -> DirectoryError {
    DirectoryError::Network(error.to_string())
}
