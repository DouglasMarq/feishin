#[derive(Debug, thiserror::Error)]
pub enum DirectoryError {
    #[error("radio directory unreachable: {0}")]
    Network(#[from] reqwest::Error),
    #[error("radio directory answered with HTTP {0}")]
    Status(u16),
    #[error("unexpected radio directory response: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("the radio directory answer is larger than {0} bytes")]
    TooLarge(usize),
    #[error("not a radio directory station id: {0}")]
    InvalidStationId(String),
}

pub type Result<T> = std::result::Result<T, DirectoryError>;
