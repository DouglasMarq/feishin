use std::sync::Arc;

use crate::protocol::RemoteFailure;

#[derive(Debug, Clone, thiserror::Error)]
pub enum RemoteError {
    #[error("network error: {0}")]
    Io(#[source] Arc<std::io::Error>),
    #[error("unexpected message: {0}")]
    Protocol(&'static str),
    #[error("unexpected message: {0}")]
    Malformed(#[source] Arc<serde_json::Error>),
    #[error("the other device did not allow the connection")]
    Denied,
    #[error("the other device did not answer in time")]
    TimedOut,
    #[error("the other device could not be reached")]
    Unreachable,
    #[error("device discovery failed: {0}")]
    Discovery(#[source] Arc<mdns_sd::Error>),
    #[error("the system random source failed: {0}")]
    Randomness(#[source] getrandom::Error),
    #[error("{0}")]
    Failed(RemoteFailure),
}

impl From<std::io::Error> for RemoteError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(Arc::new(error))
    }
}

impl From<serde_json::Error> for RemoteError {
    fn from(error: serde_json::Error) -> Self {
        Self::Malformed(Arc::new(error))
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;

    #[test]
    fn given_a_network_failure_the_source_is_the_io_error() {
        let error = RemoteError::from(std::io::Error::from(std::io::ErrorKind::ConnectionReset));
        let source = error.source().unwrap();
        assert_eq!(
            source.downcast_ref::<Arc<std::io::Error>>().unwrap().kind(),
            std::io::ErrorKind::ConnectionReset
        );
    }

    #[test]
    fn given_a_malformed_message_the_source_is_the_json_error() {
        let error = RemoteError::from(serde_json::from_str::<u8>("{").unwrap_err());
        let source = error.source().unwrap();
        assert!(source.downcast_ref::<Arc<serde_json::Error>>().is_some());
    }
}
