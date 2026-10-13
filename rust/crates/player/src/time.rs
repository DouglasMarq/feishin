pub(crate) fn seconds_to_milliseconds(seconds: f64) -> u64 {
    Some(seconds)
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
        .map_or(0, |seconds| (seconds * 1000.0).round() as u64)
}

pub(crate) fn mpv_seconds_from_milliseconds(milliseconds: u64) -> String {
    format!("{}.{:03}", milliseconds / 1000, milliseconds % 1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_seconds_to_milliseconds() {
        assert_eq!(seconds_to_milliseconds(1.2345), 1235);
        assert_eq!(seconds_to_milliseconds(0.0), 0);
        assert_eq!(seconds_to_milliseconds(-3.0), 0);
        assert_eq!(seconds_to_milliseconds(f64::NAN), 0);
        assert_eq!(seconds_to_milliseconds(f64::INFINITY), 0);
    }

    #[test]
    fn formats_milliseconds_as_mpv_seconds() {
        assert_eq!(mpv_seconds_from_milliseconds(12_345), "12.345");
        assert_eq!(mpv_seconds_from_milliseconds(4_005), "4.005");
        assert_eq!(mpv_seconds_from_milliseconds(0), "0.000");
    }
}
