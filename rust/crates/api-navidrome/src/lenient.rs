use serde::{Deserialize, Deserializer};
use serde_json::Value;

pub(crate) fn optional_string<'de, Input: Deserializer<'de>>(
    deserializer: Input,
) -> Result<Option<String>, Input::Error> {
    Ok(match Option::<Value>::deserialize(deserializer)? {
        Some(Value::String(text)) => Some(text),
        Some(Value::Number(number)) => Some(number.to_string()),
        Some(Value::Bool(flag)) => Some(flag.to_string()),
        _ => None,
    })
}

pub(crate) fn string<'de, Input: Deserializer<'de>>(
    deserializer: Input,
) -> Result<String, Input::Error> {
    Ok(optional_string(deserializer)?.unwrap_or_default())
}

pub(crate) fn optional_f64<'de, Input: Deserializer<'de>>(
    deserializer: Input,
) -> Result<Option<f64>, Input::Error> {
    Ok(match Option::<Value>::deserialize(deserializer)? {
        Some(Value::Number(number)) => number.as_f64(),
        Some(Value::String(text)) => text.trim().parse().ok(),
        _ => None,
    })
}

pub(crate) fn optional_u32<'de, Input: Deserializer<'de>>(
    deserializer: Input,
) -> Result<Option<u32>, Input::Error> {
    Ok(optional_f64(deserializer)?
        .filter(|number| *number >= 0.0)
        .map(|number| number as u32))
}

pub(crate) fn optional_i32<'de, Input: Deserializer<'de>>(
    deserializer: Input,
) -> Result<Option<i32>, Input::Error> {
    Ok(optional_f64(deserializer)?.map(|number| number as i32))
}

pub(crate) fn optional_u64<'de, Input: Deserializer<'de>>(
    deserializer: Input,
) -> Result<Option<u64>, Input::Error> {
    Ok(optional_f64(deserializer)?
        .filter(|number| *number >= 0.0)
        .map(|number| number as u64))
}

pub(crate) fn optional_f32<'de, Input: Deserializer<'de>>(
    deserializer: Input,
) -> Result<Option<f32>, Input::Error> {
    Ok(optional_f64(deserializer)?.map(|number| number as f32))
}

pub(crate) fn truthy<'de, Input: Deserializer<'de>>(
    deserializer: Input,
) -> Result<bool, Input::Error> {
    Ok(match Option::<Value>::deserialize(deserializer)? {
        Some(Value::Bool(flag)) => flag,
        Some(Value::String(text)) => !text.is_empty(),
        Some(Value::Number(number)) => number.as_f64().is_some_and(|number| number != 0.0),
        Some(Value::Array(_) | Value::Object(_)) => true,
        Some(Value::Null) | None => false,
    })
}
