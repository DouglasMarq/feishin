use std::path::Path;

use crate::{Result, none_when_missing, write_atomic};

pub fn read(path: &Path) -> Result<Option<Vec<u8>>> {
    none_when_missing(std::fs::read(path))
}

pub fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    write_atomic(path, bytes)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[test]
    fn missing_files_read_as_none_and_written_files_are_private() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state/devices.json");
        assert_eq!(read(&path).unwrap(), None);
        write(&path, b"{}").unwrap();
        assert_eq!(read(&path).unwrap().as_deref(), Some(b"{}".as_slice()));
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
