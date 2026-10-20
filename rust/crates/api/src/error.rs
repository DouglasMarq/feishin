pub type ErrorCause = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("network error: {0}")]
    Network(#[source] ErrorCause),
    #[error("network error: invalid server address: {0}")]
    InvalidAddress(#[source] ErrorCause),
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("not authorized: {0}")]
    NotAuthorized(String),
    #[error("not found")]
    NotFound,
    #[error("server error {code}: {message}")]
    Server { code: i32, message: String },
    #[error("unexpected response: {0}")]
    Decode(#[source] ErrorCause),
    #[error("not supported by this server: {0}")]
    Unsupported(&'static str),
}

pub type Result<T> = std::result::Result<T, ApiError>;

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;

    #[test]
    fn given_a_decode_failure_the_source_is_the_parse_error() {
        let parse_error = serde_json::from_str::<u32>("not a number").unwrap_err();
        let error = ApiError::Decode(Box::new(parse_error));
        let source = error.source().unwrap();
        assert!(source.downcast_ref::<serde_json::Error>().is_some());
        assert_eq!(error.to_string(), format!("unexpected response: {source}"));
    }

    #[test]
    fn given_a_network_failure_the_source_is_the_transport_error() {
        let refused = std::io::Error::from(std::io::ErrorKind::ConnectionRefused);
        let error = ApiError::Network(Box::new(refused));
        let source = error.source().unwrap();
        assert!(source.downcast_ref::<std::io::Error>().is_some());
    }
}
