const MAX_RATING: u32 = 5;

pub fn non_negative_u32(value: Option<f64>) -> Option<u32> {
    value
        .filter(|number| number.is_finite() && *number >= 0.0)
        .map(|number| number.min(f64::from(u32::MAX)) as u32)
}

pub fn positive_u32(value: Option<f64>) -> Option<u32> {
    non_negative_u32(value).filter(|number| *number > 0)
}

pub fn seconds_to_milliseconds(seconds: Option<f64>) -> u64 {
    seconds
        .filter(|number| number.is_finite() && *number > 0.0)
        .map_or(0, |number| (number * 1000.0).round() as u64)
}

pub fn nonzero_year(value: Option<f64>) -> Option<i32> {
    value
        .filter(|number| number.is_finite() && *number != 0.0)
        .map(|number| number as i32)
}

pub fn capped_rating(value: Option<f64>) -> Option<u8> {
    positive_u32(value).map(|rating| rating.min(MAX_RATING) as u8)
}

pub fn non_negative_size(value: Option<f64>) -> Option<u64> {
    value
        .filter(|number| number.is_finite() && *number >= 0.0)
        .map(|number| number as u64)
}

pub fn single_precision(value: Option<f64>) -> Option<f32> {
    value.map(|number| number as f32)
}

pub fn non_empty_text(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.is_empty())
}
