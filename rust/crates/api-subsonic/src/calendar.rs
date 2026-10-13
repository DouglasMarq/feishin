use std::time::{SystemTime, UNIX_EPOCH};

const SECONDS_PER_DAY: u64 = 86_400;
const DAYS_FROM_YEAR_ZERO_MARCH_TO_EPOCH: i64 = 719_468;
const DAYS_PER_ERA: i64 = 146_097;
const YEARS_PER_ERA: i64 = 400;
const MARCH_BASED_JANUARY: i64 = 10;

fn days_since_unix_epoch() -> i64 {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    i64::try_from(seconds / SECONDS_PER_DAY).unwrap_or(0)
}

fn civil_year_from_days(days_since_epoch: i64) -> i64 {
    let days_since_march_epoch = days_since_epoch + DAYS_FROM_YEAR_ZERO_MARCH_TO_EPOCH;
    let era = days_since_march_epoch.div_euclid(DAYS_PER_ERA);
    let day_of_era = days_since_march_epoch.rem_euclid(DAYS_PER_ERA);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let march_based_month = (5 * day_of_year + 2) / 153;
    let is_january_or_february = march_based_month >= MARCH_BASED_JANUARY;
    year_of_era + era * YEARS_PER_ERA + i64::from(is_january_or_february)
}

pub fn current_year() -> i32 {
    i32::try_from(civil_year_from_days(days_since_unix_epoch())).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_year_is_plausible() {
        let year = current_year();
        assert!((2024..3000).contains(&year));
    }

    #[test]
    fn civil_year_handles_year_boundaries() {
        assert_eq!(civil_year_from_days(0), 1970);
        assert_eq!(civil_year_from_days(364), 1970);
        assert_eq!(civil_year_from_days(365), 1971);
        assert_eq!(civil_year_from_days(19_782), 2024);
        assert_eq!(civil_year_from_days(19_783), 2024);
        assert_eq!(civil_year_from_days(-1), 1969);
    }
}
