use std::time::Duration;

use super::schedule::{FadeWindow, LONGEST_RECHECK};
use crate::deck::DeckClock;

const STANDBY_LOAD_LEAD_SECONDS: f64 = 15.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StandbyLoad {
    Now,
    Later(Duration),
}

pub(crate) fn plan_standby_load(crossfade: Duration, lead: DeckClock) -> StandbyLoad {
    let Some(position_seconds) = lead.position_seconds else {
        return StandbyLoad::Later(LONGEST_RECHECK);
    };
    let Some(window) = FadeWindow::planned(lead.duration_seconds, crossfade) else {
        return StandbyLoad::Now;
    };
    let load_at_seconds = window.start_seconds() - STANDBY_LOAD_LEAD_SECONDS;
    match Duration::try_from_secs_f64(load_at_seconds - position_seconds) {
        Ok(wait) if !wait.is_zero() => StandbyLoad::Later(wait.min(LONGEST_RECHECK)),
        _ => StandbyLoad::Now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIVE_SECONDS: Duration = Duration::from_secs(5);

    fn clock(position_seconds: Option<f64>, duration_seconds: Option<f64>) -> DeckClock {
        DeckClock {
            position_seconds,
            duration_seconds,
            is_paused: false,
        }
    }

    #[test]
    fn next_track_loads_only_shortly_before_the_fade() {
        let load_at = FadeWindow::planned(Some(300.0), FIVE_SECONDS)
            .map_or(0.0, FadeWindow::start_seconds)
            - STANDBY_LOAD_LEAD_SECONDS;
        let shortly_before = load_at - 0.25;
        assert!((279.0..280.0).contains(&load_at));
        assert_eq!(
            plan_standby_load(FIVE_SECONDS, clock(Some(10.0), Some(300.0))),
            StandbyLoad::Later(LONGEST_RECHECK)
        );
        assert_eq!(
            plan_standby_load(FIVE_SECONDS, clock(Some(shortly_before), Some(300.0))),
            StandbyLoad::Later(Duration::from_secs_f64(load_at - shortly_before))
        );
        for due in [load_at, 299.0, 400.0] {
            assert_eq!(
                plan_standby_load(FIVE_SECONDS, clock(Some(due), Some(300.0))),
                StandbyLoad::Now
            );
        }
    }

    #[test]
    fn short_tracks_load_the_next_one_at_once() {
        assert_eq!(
            plan_standby_load(FIVE_SECONDS, clock(Some(0.0), Some(20.0))),
            StandbyLoad::Now
        );
    }

    #[test]
    fn unknown_length_loads_at_once_and_unknown_position_waits() {
        assert_eq!(
            plan_standby_load(FIVE_SECONDS, clock(Some(10.0), None)),
            StandbyLoad::Now
        );
        for duration in [Some(300.0), None] {
            assert_eq!(
                plan_standby_load(FIVE_SECONDS, clock(None, duration)),
                StandbyLoad::Later(LONGEST_RECHECK)
            );
        }
    }
}
