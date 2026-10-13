use std::path::Path;

use rusty_leveldb::{DB, LdbIterator, Options};
use tempfile::TempDir;

use super::STORE_KEY;
use crate::{Result, StorageError};

const PREFERRED_ORIGIN: &str = "file://";
const UTF16_ENCODING_MARKER: u8 = 0;
const LATIN1_ENCODING_MARKER: u8 = 1;

struct StoreEntry {
    origin: String,
    value: Vec<u8>,
}

pub(super) fn read_store_value(leveldb_dir: &Path) -> Result<Option<String>> {
    let database_copy = copy_database(leveldb_dir)?;
    find_store_value(database_copy.path())?
        .map(|value| {
            decode_chromium_string(&value)
                .ok_or_else(|| StorageError::Invalid(format!("{STORE_KEY}: bad value encoding")))
        })
        .transpose()
}

fn copy_database(leveldb_dir: &Path) -> Result<TempDir> {
    let database_copy = tempfile::tempdir()?;
    for entry in std::fs::read_dir(leveldb_dir)? {
        let entry = entry?;
        if entry.file_name() == "LOCK" || !entry.file_type()?.is_file() {
            continue;
        }
        std::fs::copy(entry.path(), database_copy.path().join(entry.file_name()))?;
    }
    Ok(database_copy)
}

fn find_store_value(database_dir: &Path) -> Result<Option<Vec<u8>>> {
    let options = Options {
        create_if_missing: false,
        ..Options::default()
    };
    let mut database = DB::open(database_dir, options).map_err(leveldb_error)?;
    let mut iterator = database.new_iter().map_err(leveldb_error)?;
    let mut found: Option<StoreEntry> = None;
    while iterator.advance() {
        let Some((key, value)) = iterator.current() else {
            continue;
        };
        let Some(origin) = store_key_origin(&key) else {
            continue;
        };
        if is_better_origin(found.as_ref(), &origin) {
            found = Some(StoreEntry {
                origin,
                value: value.to_vec(),
            });
        }
    }
    Ok(found.map(|entry| entry.value))
}

fn store_key_origin(key: &[u8]) -> Option<String> {
    let (origin, script_key) = split_local_storage_key(key)?;
    (decode_chromium_string(script_key).as_deref() == Some(STORE_KEY)).then_some(origin)
}

fn is_better_origin(current: Option<&StoreEntry>, origin: &str) -> bool {
    match current {
        None => true,
        Some(current) => current.origin != PREFERRED_ORIGIN && origin == PREFERRED_ORIGIN,
    }
}

fn split_local_storage_key(key: &[u8]) -> Option<(String, &[u8])> {
    let mut parts = key.strip_prefix(b"_")?.splitn(2, |&byte| byte == 0);
    let origin = String::from_utf8_lossy(parts.next()?).into_owned();
    let script_key = parts.next()?;
    Some((origin, script_key))
}

pub(super) fn decode_chromium_string(bytes: &[u8]) -> Option<String> {
    let (&encoding_marker, data) = bytes.split_first()?;
    match encoding_marker {
        UTF16_ENCODING_MARKER => decode_utf16_le(data),
        LATIN1_ENCODING_MARKER => Some(data.iter().map(|&byte| char::from(byte)).collect()),
        _ => None,
    }
}

fn decode_utf16_le(data: &[u8]) -> Option<String> {
    let (pairs, remainder) = data.as_chunks::<2>();
    if !remainder.is_empty() {
        return None;
    }
    let units: Vec<u16> = pairs.iter().map(|&pair| u16::from_le_bytes(pair)).collect();
    Some(String::from_utf16_lossy(&units))
}

fn leveldb_error(status: rusty_leveldb::Status) -> StorageError {
    StorageError::Invalid(format!("Feishin localStorage: {status}"))
}
