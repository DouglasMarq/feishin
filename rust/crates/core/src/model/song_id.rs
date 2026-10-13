use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SongId(Box<str>);

impl SongId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for SongId {
    fn from(id: &str) -> Self {
        Self(Box::from(id))
    }
}

impl From<String> for SongId {
    fn from(id: String) -> Self {
        Self(id.into_boxed_str())
    }
}

impl AsRef<str> for SongId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<str> for SongId {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for SongId {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl fmt::Display for SongId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
