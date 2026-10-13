use feishin_core::model::ReplayGain;

const MAX_RATING: u32 = 5;
const PARTIAL_ISO_DATE_SHAPE: &[u8] = b"dddd-dd-dd";
const DIGIT_PLACEHOLDER: u8 = b'd';
const UNSET_PLAY_DATE_MARKER: &str = "0001-";

pub(crate) fn seconds_to_ms(seconds: Option<f64>) -> u64 {
    seconds
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
        .map(|seconds| (seconds * 1000.0).round() as u64)
        .unwrap_or(0)
}

pub(crate) fn rating(value: Option<u32>) -> Option<u8> {
    value
        .filter(|rating| *rating > 0)
        .map(|rating| rating.min(MAX_RATING) as u8)
}

pub(crate) fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.is_empty())
}

pub(crate) fn partial_iso_year(input: Option<&str>) -> Option<i32> {
    let trimmed = input?.trim();
    if !matches_partial_iso_date(trimmed.as_bytes()) {
        return None;
    }
    trimmed.get(0..4)?.parse().ok()
}

fn matches_partial_iso_date(bytes: &[u8]) -> bool {
    matches!(bytes.len(), 4 | 7 | 10)
        && bytes
            .iter()
            .zip(PARTIAL_ISO_DATE_SHAPE)
            .all(|(byte, expected)| match *expected {
                DIGIT_PLACEHOLDER => byte.is_ascii_digit(),
                separator => *byte == separator,
            })
}

pub(crate) fn play_date(value: Option<String>) -> Option<String> {
    value.filter(|date| !date.is_empty() && !date.contains(UNSET_PLAY_DATE_MARKER))
}

pub(crate) struct ReplayGainValues {
    pub album_gain: Option<f32>,
    pub album_peak: Option<f32>,
    pub track_gain: Option<f32>,
    pub track_peak: Option<f32>,
}

pub(crate) fn replay_gain(values: ReplayGainValues) -> Option<ReplayGain> {
    let is_set = |value: Option<f32>| value.is_some_and(|number| number != 0.0);
    let has_gain = is_set(values.album_gain) || is_set(values.track_gain);
    let has_peak = is_set(values.album_peak) || is_set(values.track_peak);
    if !has_gain && !has_peak {
        return None;
    }
    Some(ReplayGain {
        track_gain_db: values.track_gain.filter(|_| has_gain),
        track_peak: values.track_peak.filter(|_| has_peak),
        album_gain_db: values.album_gain.filter(|_| has_gain),
        album_peak: values.album_peak.filter(|_| has_peak),
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
        let track_gain_zero = ReplayGainValues {
            album_gain: None,
            album_peak: None,
            track_gain: Some(0.0),
            track_peak: None,
        };
        assert_eq!(replay_gain(track_gain_zero), None);
        let album_gain_and_track_peak = ReplayGainValues {
            album_gain: Some(-6.0),
            album_peak: None,
            track_gain: None,
            track_peak: Some(0.9),
        };
        assert_eq!(
            replay_gain(album_gain_and_track_peak),
            Some(ReplayGain {
                track_gain_db: None,
                track_peak: Some(0.9),
                album_gain_db: Some(-6.0),
                album_peak: None,
            })
        );
    }
}
