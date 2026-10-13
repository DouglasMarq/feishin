#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid data: {0}")]
    Invalid(String),
    #[error("keyring error: {0}")]
    Keyring(String),
    #[error("network error: {0}")]
    Network(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;

impl StorageError {
    pub(crate) fn duplicate(&self) -> Self {
        match self {
            Self::Io(io_error) => {
                Self::Io(std::io::Error::new(io_error.kind(), io_error.to_string()))
            }
            Self::Invalid(message) => Self::Invalid(message.clone()),
            Self::Keyring(message) => Self::Keyring(message.clone()),
            Self::Network(message) => Self::Network(message.clone()),
        }
    }
}
