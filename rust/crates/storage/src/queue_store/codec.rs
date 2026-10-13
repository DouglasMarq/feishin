use bincode::config::{self, Config};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{Result, StorageError};

const MAX_DECODED_BYTES: usize = 512 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FileFormat {
    pub(super) file_name: &'static str,
    pub(super) magic: [u8; 4],
}

impl FileFormat {
    pub(super) fn encode(self, value: &impl Serialize) -> Result<Vec<u8>> {
        let mut bytes = self.magic.to_vec();
        bincode::serde::encode_into_std_write(value, &mut bytes, config::standard())
            .map_err(|error| self.invalid(error))?;
        Ok(bytes)
    }

    pub(super) fn decode<Value: DeserializeOwned>(self, bytes: &[u8]) -> Result<Value> {
        let payload = bytes
            .strip_prefix(self.magic.as_slice())
            .ok_or_else(|| self.invalid("unknown file format"))?;
        let (value, consumed) = bincode::serde::decode_from_slice(payload, decoding_config())
            .map_err(|error| self.invalid(error))?;
        match consumed == payload.len() {
            true => Ok(value),
            false => Err(self.invalid("unexpected trailing bytes")),
        }
    }

    pub(super) fn invalid(self, error: impl std::fmt::Display) -> StorageError {
        StorageError::Invalid(format!("{}: {error}", self.file_name))
    }
}

fn decoding_config() -> impl Config {
    config::standard().with_limit::<MAX_DECODED_BYTES>()
}
