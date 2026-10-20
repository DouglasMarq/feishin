use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const SECONDS_PER_DAY: u64 = 86_400;
const DAYS_PER_WEEK: u32 = 7;
const DAYS_FROM_EPOCH_THURSDAY_TO_MONDAY: u32 = 3;
const SEED_MULTIPLIER: u64 = 0x9E37_79B9;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixDay(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixWeek(pub u32);

impl MixDay {
    pub fn today() -> Self {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or_default();
        Self::from_unix_seconds(seconds)
    }

    pub fn from_unix_seconds(seconds: u64) -> Self {
        Self(u32::try_from(seconds / SECONDS_PER_DAY).unwrap_or(u32::MAX))
    }

    pub fn week(self) -> MixWeek {
        MixWeek(self.0.saturating_add(DAYS_FROM_EPOCH_THURSDAY_TO_MONDAY) / DAYS_PER_WEEK)
    }

    pub fn shuffle_seed(self, salt: u64) -> u64 {
        u64::from(self.0)
            .wrapping_mul(SEED_MULTIPLIER)
            .wrapping_add(salt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONDAY_2026_10_19: u64 = 1_792_368_000;

    #[test]
    fn days_count_from_the_epoch_in_utc() {
        assert_eq!(MixDay::from_unix_seconds(0), MixDay(0));
        assert_eq!(MixDay::from_unix_seconds(SECONDS_PER_DAY - 1), MixDay(0));
        assert_eq!(MixDay::from_unix_seconds(SECONDS_PER_DAY), MixDay(1));
    }

    #[test]
    fn weeks_start_on_monday() {
        let monday = MixDay::from_unix_seconds(MONDAY_2026_10_19);
        let sunday = MixDay(monday.0 - 1);
        let next_sunday = MixDay(monday.0 + 6);
        assert_eq!(monday.week(), next_sunday.week());
        assert_ne!(sunday.week(), monday.week());
        assert_eq!(MixDay(4).week(), MixWeek(1), "1970-01-05 was a Monday");
    }

    #[test]
    fn each_day_and_salt_shuffles_differently() {
        assert_ne!(MixDay(1).shuffle_seed(0), MixDay(2).shuffle_seed(0));
        assert_ne!(MixDay(1).shuffle_seed(0), MixDay(1).shuffle_seed(1));
        assert_eq!(MixDay(1).shuffle_seed(3), MixDay(1).shuffle_seed(3));
    }
}
