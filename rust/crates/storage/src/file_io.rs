use std::io::{ErrorKind, Write};
use std::path::Path;

use crate::{Result, StorageError};

pub(crate) const TEMPORARY_FILE_PREFIX: &str = ".tmp-";
const TEMPORARY_FILE_SUFFIX: &str = ".part";

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let directory = parent_directory_or_current(path);
    std::fs::create_dir_all(directory)?;
    let mut temporary_file = tempfile::Builder::new()
        .prefix(TEMPORARY_FILE_PREFIX)
        .suffix(TEMPORARY_FILE_SUFFIX)
        .tempfile_in(directory)?;
    temporary_file.write_all(bytes)?;
    temporary_file.as_file().sync_all()?;
    temporary_file
        .persist(path)
        .map_err(|persist_error| StorageError::Io(persist_error.error))?;
    Ok(())
}

pub(crate) fn none_when_missing<T>(io_result: std::io::Result<T>) -> Result<Option<T>> {
    match io_result {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn parent_directory_or_current(path: &Path) -> &Path {
    path.parent()
        .filter(|directory| !directory.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::*;

    #[test]
    fn write_atomic_replaces_and_leaves_no_temp_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nested/file.txt");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        let entries: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(entries, vec![OsString::from("file.txt")]);
    }
}
