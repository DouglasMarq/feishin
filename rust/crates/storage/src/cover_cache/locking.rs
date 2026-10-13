use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::{Result, StorageError};

pub(super) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(super) async fn run_blocking<T: Send + 'static>(
    blocking_work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(blocking_work)
        .await
        .map_err(|join_error| {
            StorageError::Invalid(format!("cover cache task failed: {join_error}"))
        })?
}
