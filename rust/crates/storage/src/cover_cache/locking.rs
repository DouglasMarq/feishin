use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::{Result, StorageError};

const COVER_CACHE_TASK: &str = "cover cache task failed";

pub(super) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(super) async fn run_blocking<T: Send + 'static>(
    blocking_work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(blocking_work)
        .await
        .map_err(|join_error| StorageError::malformed(COVER_CACHE_TASK, join_error))?
}
