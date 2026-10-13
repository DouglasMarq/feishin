use serde::{Deserialize, Deserializer};
use serde_json::Value;

pub fn lenient_id<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Ok(lenient_optional_id(deserializer)?.unwrap_or_default())
}

pub fn lenient_optional_id<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::String(text) => Some(text),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    })
}

pub fn lenient_number<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<f64>, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    })
}
