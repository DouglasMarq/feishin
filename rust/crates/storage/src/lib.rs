mod error;
mod file_io;
mod paths;

pub mod cover_cache;
pub mod credentials;
pub mod feishin_import;
pub mod queue_store;
pub mod settings;

pub use error::{Result, StorageError};
pub use paths::{APP_DIR_NAME, Paths};

pub(crate) use file_io::{TEMPORARY_FILE_PREFIX, none_when_missing, write_atomic};
pub(crate) use paths::xdg_base;
