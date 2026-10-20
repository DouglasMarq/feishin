use crate::protocol::RemoteFailure;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RemoteError {
    #[error("network error: {0}")]
    Io(String),
    #[error("unexpected message: {0}")]
    Protocol(String),
    #[error("the other device did not allow the connection")]
    Denied,
    #[error("the other device did not answer in time")]
    TimedOut,
    #[error("device discovery failed: {0}")]
    Discovery(String),
    #[error("{0}")]
    Failed(RemoteFailure),
}
