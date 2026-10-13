//! Server credentials in the system keyring (Secret Service on Linux), keyed by server id.

use crate::Result;

pub fn set(_server_id: &str, _credential: &str) -> Result<()> {
    todo!()
}

pub fn get(_server_id: &str) -> Result<Option<String>> {
    todo!()
}

pub fn delete(_server_id: &str) -> Result<()> {
    todo!()
}
