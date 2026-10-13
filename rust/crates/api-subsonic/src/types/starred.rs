use serde::{Deserialize, Deserializer};
use serde_json::Value;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum Starred {
    #[default]
    Missing,
    Timestamp(String),
    Flag(bool),
    OtherValue,
}

impl Starred {
    pub fn is_starred(&self) -> bool {
        match self {
            Self::Missing => false,
            Self::Timestamp(timestamp) => !timestamp.is_empty(),
            Self::Flag(flag) => *flag,
            Self::OtherValue => true,
        }
    }

    pub fn sort_value(&self) -> Option<String> {
        match self {
            Self::Timestamp(timestamp) => Some(timestamp.clone()),
            other => other.is_starred().then(String::new),
        }
    }
}

impl<'de> Deserialize<'de> for Starred {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match Value::deserialize(deserializer)? {
            Value::Null => Self::Missing,
            Value::String(timestamp) => Self::Timestamp(timestamp),
            Value::Bool(flag) => Self::Flag(flag),
            Value::Number(_) | Value::Array(_) | Value::Object(_) => Self::OtherValue,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn starred_from(json: &str) -> Starred {
        serde_json::from_str(json).unwrap_or_default()
    }

    #[test]
    fn starred_matches_javascript_truthiness() {
        assert!(starred_from("\"2024-01-01\"").is_starred());
        assert!(!starred_from("\"\"").is_starred());
        assert!(starred_from("true").is_starred());
        assert!(!starred_from("false").is_starred());
        assert!(!starred_from("null").is_starred());
        assert!(starred_from("0").is_starred());
        assert!(!Starred::Missing.is_starred());
    }

    #[test]
    fn sort_value_keeps_timestamps_and_marks_other_starred_values() {
        assert_eq!(
            starred_from("\"2024-01-01\"").sort_value().as_deref(),
            Some("2024-01-01")
        );
        assert_eq!(starred_from("true").sort_value().as_deref(), Some(""));
        assert_eq!(starred_from("false").sort_value(), None);
        assert_eq!(Starred::Missing.sort_value(), None);
    }
}
