use std::sync::Arc;

pub type ErrorCause = Arc<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum StorageError {
    #[error("io error: {0}")]
    Io(#[source] Arc<std::io::Error>),
    #[error("invalid data: {0}")]
    Invalid(String),
    #[error("invalid data: {context}: {source}")]
    Malformed {
        context: &'static str,
        #[source]
        source: ErrorCause,
    },
    #[error("keyring error: {0}")]
    Keyring(#[source] Arc<keyring_core::Error>),
    #[error("network error: {0}")]
    Network(#[source] Arc<reqwest::Error>),
}

pub type Result<T> = std::result::Result<T, StorageError>;

impl From<std::io::Error> for StorageError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(Arc::new(error))
    }
}

impl StorageError {
    pub(crate) fn malformed(
        context: &'static str,
        error: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Malformed {
            context,
            source: Arc::new(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;

    #[test]
    fn given_an_io_failure_the_source_is_the_io_error() {
        let error = StorageError::from(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        let source = error.source().unwrap();
        assert_eq!(
            source.downcast_ref::<Arc<std::io::Error>>().unwrap().kind(),
            std::io::ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn given_malformed_data_the_source_is_the_parse_error_and_the_text_names_the_file() {
        let parse_error = serde_json::from_str::<u32>("[").unwrap_err();
        let parse_text = parse_error.to_string();
        let error = StorageError::malformed("queue.json", parse_error);
        let cause = error
            .source()
            .unwrap()
            .downcast_ref::<ErrorCause>()
            .unwrap();
        assert!(cause.downcast_ref::<serde_json::Error>().is_some());
        assert_eq!(
            error.to_string(),
            format!("invalid data: queue.json: {parse_text}")
        );
    }

    #[test]
    fn given_a_clone_the_source_is_shared() {
        let error = StorageError::from(std::io::Error::other("disk full"));
        let copy = error.clone();
        assert_eq!(copy.to_string(), "io error: disk full");
        let StorageError::Io(original) = &error else {
            panic!("an io error");
        };
        let shared = copy.source().unwrap().downcast_ref::<Arc<std::io::Error>>();
        assert!(shared.is_some_and(|shared| Arc::ptr_eq(shared, original)));
    }
}
