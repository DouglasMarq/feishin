#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("network error: {0}")]
    Network(String),
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("not found")]
    NotFound,
    #[error("server error {code}: {message}")]
    Server { code: i32, message: String },
    #[error("unexpected response: {0}")]
    Decode(String),
    #[error("not supported by this server: {0}")]
    Unsupported(&'static str),
}

pub type Result<T> = std::result::Result<T, ApiError>;
