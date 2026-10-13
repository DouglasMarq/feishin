use feishin_core::model::ReplayGain;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// Accepts a string, a number or null.
pub(crate) fn opt_string<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(match Option::<Value>::deserialize(d)? {
        Some(Value::String(s)) => Some(s),
        Some(Value::Number(n)) => Some(n.to_string()),
        Some(Value::Bool(b)) => Some(b.to_string()),
        _ => None,
    })
}

pub(crate) fn string<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(opt_string(d)?.unwrap_or_default())
}

/// Accepts a number, a numeric string or null.
pub(crate) fn opt_f64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    Ok(match Option::<Value>::deserialize(d)? {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.trim().parse().ok(),
        _ => None,
    })
}

pub(crate) fn opt_u32<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u32>, D::Error> {
    Ok(opt_f64(d)?.filter(|v| *v >= 0.0).map(|v| v as u32))
}

pub(crate) fn opt_i32<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i32>, D::Error> {
    Ok(opt_f64(d)?.map(|v| v as i32))
}

pub(crate) fn opt_u64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
    Ok(opt_f64(d)?.filter(|v| *v >= 0.0).map(|v| v as u64))
}

pub(crate) fn opt_f32<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f32>, D::Error> {
    Ok(opt_f64(d)?.map(|v| v as f32))
}

/// JavaScript-like truthiness: Subsonic sends `starred` as a date string, Navidrome as a bool.
pub(crate) fn truthy<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    Ok(match Option::<Value>::deserialize(d)? {
        Some(Value::Bool(b)) => b,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Number(n)) => n.as_f64().is_some_and(|v| v != 0.0),
        Some(Value::Array(_) | Value::Object(_)) => true,
        Some(Value::Null) | None => false,
    })
}

pub(crate) fn seconds_to_ms(seconds: Option<f64>) -> u64 {
    seconds
        .filter(|s| s.is_finite() && *s > 0.0)
        .map(|s| (s * 1000.0).round() as u64)
        .unwrap_or(0)
}

pub(crate) fn rating(value: Option<u32>) -> Option<u8> {
    value.filter(|r| *r > 0).map(|r| r.min(5) as u8)
}

pub(crate) fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|s| !s.is_empty())
}

/// Parses `YYYY`, `YYYY-MM` or `YYYY-MM-DD` and returns the year.
pub(crate) fn partial_iso_year(input: Option<&str>) -> Option<i32> {
    let s = input?.trim();
    let bytes = s.as_bytes();
    let digits = |range: std::ops::Range<usize>| bytes[range].iter().all(u8::is_ascii_digit);
    let valid = match bytes.len() {
        4 => digits(0..4),
        7 => digits(0..4) && bytes[4] == b'-' && digits(5..7),
        10 => digits(0..4) && bytes[4] == b'-' && digits(5..7) && bytes[7] == b'-' && digits(8..10),
        _ => false,
    };
    if !valid {
        return None;
    }
    s.get(0..4)?.parse().ok()
}

pub(crate) fn play_date(value: Option<String>) -> Option<String> {
    value.filter(|d| !d.is_empty() && !d.contains("0001-"))
}

pub(crate) fn replay_gain(
    album_gain: Option<f32>,
    album_peak: Option<f32>,
    track_gain: Option<f32>,
    track_peak: Option<f32>,
) -> Option<ReplayGain> {
    let set = |v: Option<f32>| v.is_some_and(|x| x != 0.0);
    let has_gain = set(album_gain) || set(track_gain);
    let has_peak = set(album_peak) || set(track_peak);
    if !has_gain && !has_peak {
        return None;
    }
    Some(ReplayGain {
        track_gain_db: track_gain.filter(|_| has_gain),
        track_peak: track_peak.filter(|_| has_peak),
        album_gain_db: album_gain.filter(|_| has_gain),
        album_peak: album_peak.filter(|_| has_peak),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_iso_dates() {
        assert_eq!(partial_iso_year(Some("2001")), Some(2001));
        assert_eq!(partial_iso_year(Some("2001-02")), Some(2001));
        assert_eq!(partial_iso_year(Some("2001-02-03")), Some(2001));
        assert_eq!(partial_iso_year(Some("2001-02-03T00:00:00Z")), None);
        assert_eq!(partial_iso_year(Some("")), None);
        assert_eq!(partial_iso_year(None), None);
    }

    #[test]
    fn replay_gain_only_when_present() {
        assert_eq!(replay_gain(None, None, Some(0.0), None), None);
        let rg = replay_gain(Some(-6.0), None, None, Some(0.9));
        assert_eq!(
            rg,
            Some(ReplayGain {
                track_gain_db: None,
                track_peak: Some(0.9),
                album_gain_db: Some(-6.0),
                album_peak: None,
            })
        );
    }
}
