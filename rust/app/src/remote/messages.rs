use feishin_remote::{RemoteError, RemoteFailure};

pub fn failure_text(failure: RemoteFailure) -> String {
    match failure {
        RemoteFailure::DifferentServer => tr!("it is connected to a different server"),
        RemoteFailure::NotConnected => tr!("it is not connected to a server"),
        RemoteFailure::NothingToTransfer => tr!("nothing is playing"),
        RemoteFailure::VersionMismatch => tr!("it runs a different version of Cadence"),
        RemoteFailure::Busy => tr!("it is busy with another request"),
        RemoteFailure::Stopped => tr!("it stopped"),
    }
}

pub fn error_text(error: &RemoteError) -> String {
    match error {
        RemoteError::Denied => tr!("the connection was not allowed"),
        RemoteError::TimedOut => tr!("it did not answer in time"),
        RemoteError::Failed(failure) => failure_text(*failure),
        RemoteError::Io(_)
        | RemoteError::Protocol(_)
        | RemoteError::Malformed(_)
        | RemoteError::Discovery(_)
        | RemoteError::Randomness(_) => {
            tr!("the network connection failed")
        }
    }
}
