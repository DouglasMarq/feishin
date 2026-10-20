use std::time::Duration;

use super::gain::{FadeGains, equal_power_gains};
use crate::deck::DeckClock;

pub(crate) const FADE_STEP: Duration = Duration::from_millis(40);
const SHORTEST_RECHECK: Duration = Duration::from_millis(5);
pub(super) const LONGEST_RECHECK: Duration = Duration::from_millis(500);
const OUTGOING_END_GRACE_SECONDS: f64 = 1.5;
pub(crate) const OUTPUT_BUFFER_SECONDS: f64 = 0.2;
const DRAIN_MARGIN_SECONDS: f64 = OUTPUT_BUFFER_SECONDS + 0.05;
const LONGEST_FADE_SHARE_OF_TRACK: f64 = 0.4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FadeWindow {
    start_seconds: f64,
    end_seconds: f64,
}

impl FadeWindow {
    pub(crate) fn planned(duration_seconds: Option<f64>, crossfade: Duration) -> Option<Self> {
        let duration_seconds = duration_seconds.filter(|seconds| seconds.is_finite())?;
        let audible_end_seconds = duration_seconds - DRAIN_MARGIN_SECONDS;
        let length_seconds = crossfade
            .as_secs_f64()
            .min(duration_seconds * LONGEST_FADE_SHARE_OF_TRACK)
            .min(audible_end_seconds);
        (length_seconds > 0.0).then_some(Self {
            start_seconds: audible_end_seconds - length_seconds,
            end_seconds: audible_end_seconds,
        })
    }

    pub(super) fn start_seconds(self) -> f64 {
        self.start_seconds
    }

    fn fitting_incoming(self, incoming_duration_seconds: Option<f64>) -> Self {
        let longest_for_incoming_seconds = incoming_duration_seconds
            .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
            .map_or(f64::INFINITY, |seconds| {
                seconds * LONGEST_FADE_SHARE_OF_TRACK
            });
        Self {
            start_seconds: self
                .start_seconds
                .max(self.end_seconds - longest_for_incoming_seconds),
            end_seconds: self.end_seconds,
        }
    }

    pub(crate) fn has_begun(self, position_seconds: f64) -> bool {
        position_seconds >= self.start_seconds
    }

    pub(crate) fn starting_at(self, position_seconds: f64) -> Self {
        Self {
            start_seconds: position_seconds.clamp(self.start_seconds, self.end_seconds),
            end_seconds: self.end_seconds,
        }
    }

    pub(crate) fn progress(self, incoming_position_seconds: f64) -> f64 {
        match self.length_seconds() > 0.0 {
            true => (incoming_position_seconds / self.length_seconds()).clamp(0.0, 1.0),
            false => 1.0,
        }
    }

    fn is_overdue(self, incoming_position_seconds: f64) -> bool {
        incoming_position_seconds >= self.length_seconds() + OUTGOING_END_GRACE_SECONDS
    }

    fn length_seconds(self) -> f64 {
        self.end_seconds - self.start_seconds
    }

    fn time_until_start(self, position_seconds: f64) -> Duration {
        Duration::try_from_secs_f64(self.start_seconds - position_seconds).unwrap_or(Duration::ZERO)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum FaderPlan {
    Recheck(Duration),
    Begin(FadeWindow),
    Ramp(FadeGains),
    Finish,
}

pub(crate) fn plan_before_fade(
    crossfade: Duration,
    lead: DeckClock,
    incoming_duration_seconds: Option<f64>,
) -> FaderPlan {
    let window = FadeWindow::planned(lead.duration_seconds, crossfade)
        .map(|window| window.fitting_incoming(incoming_duration_seconds));
    match (lead.position_seconds, window) {
        (Some(position_seconds), Some(window)) if !lead.is_paused => {
            match window.has_begun(position_seconds) {
                true => FaderPlan::Begin(window.starting_at(position_seconds)),
                false => FaderPlan::Recheck(
                    window
                        .time_until_start(position_seconds)
                        .clamp(SHORTEST_RECHECK, LONGEST_RECHECK),
                ),
            }
        }
        _ => FaderPlan::Recheck(LONGEST_RECHECK),
    }
}

pub(crate) fn plan_during_fade(
    window: FadeWindow,
    incoming_position_seconds: Option<f64>,
) -> FaderPlan {
    match incoming_position_seconds {
        Some(position_seconds) if !window.is_overdue(position_seconds) => {
            FaderPlan::Ramp(equal_power_gains(window.progress(position_seconds)))
        }
        _ => FaderPlan::Finish,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIX_SECONDS: Duration = Duration::from_secs(6);

    fn clock(position_seconds: f64, duration_seconds: f64) -> DeckClock {
        DeckClock {
            position_seconds: Some(position_seconds),
            duration_seconds: Some(duration_seconds),
            is_paused: false,
        }
    }

    fn window(start_seconds: f64, end_seconds: f64) -> FadeWindow {
        FadeWindow {
            start_seconds,
            end_seconds,
        }
    }

    fn audible_end(duration: f64) -> f64 {
        duration - DRAIN_MARGIN_SECONDS
    }

    #[test]
    fn window_covers_the_last_seconds_before_the_output_drains() {
        let end = audible_end(200.0);
        assert!((199.7..199.8).contains(&end));
        assert_eq!(
            FadeWindow::planned(Some(200.0), SIX_SECONDS),
            Some(window(end - 6.0, end))
        );
    }

    #[test]
    fn window_leaves_more_than_half_of_a_short_track_unfaded() {
        let end = audible_end(10.0);
        let planned = FadeWindow::planned(Some(10.0), SIX_SECONDS);
        assert_eq!(planned, Some(window(end - 4.0, end)));
        assert!(planned.is_some_and(|fade| fade.start_seconds > 5.0));
    }

    #[test]
    fn no_window_without_a_usable_duration_or_crossfade() {
        assert_eq!(FadeWindow::planned(None, SIX_SECONDS), None);
        assert_eq!(FadeWindow::planned(Some(0.0), SIX_SECONDS), None);
        assert_eq!(FadeWindow::planned(Some(0.2), SIX_SECONDS), None);
        assert_eq!(FadeWindow::planned(Some(f64::NAN), SIX_SECONDS), None);
        assert_eq!(FadeWindow::planned(Some(f64::INFINITY), SIX_SECONDS), None);
        assert_eq!(FadeWindow::planned(Some(200.0), Duration::ZERO), None);
    }

    #[test]
    fn progress_follows_the_incoming_track_over_the_window_length() {
        let fade = window(194.0, 200.0);
        assert_eq!(fade.progress(-1.0), 0.0);
        assert_eq!(fade.progress(0.0), 0.0);
        assert_eq!(fade.progress(3.0), 0.5);
        assert_eq!(fade.progress(6.0), 1.0);
        assert_eq!(fade.progress(30.0), 1.0);
        assert_eq!(window(200.0, 200.0).progress(0.0), 1.0);
    }

    #[test]
    fn late_start_shortens_the_fade() {
        let fade = window(194.0, 200.0);
        assert_eq!(fade.starting_at(197.0), window(197.0, 200.0));
        assert_eq!(fade.starting_at(150.0), fade);
        assert_eq!(fade.starting_at(250.0), window(200.0, 200.0));
    }

    #[test]
    fn waits_until_the_window_then_begins() {
        let end = audible_end(200.0);
        let start = end - 6.0;
        assert_eq!(
            plan_before_fade(SIX_SECONDS, clock(100.0, 200.0), None),
            FaderPlan::Recheck(LONGEST_RECHECK)
        );
        assert_eq!(
            plan_before_fade(SIX_SECONDS, clock(start - 0.1, 200.0), None),
            FaderPlan::Recheck(Duration::from_secs_f64(start - (start - 0.1)))
        );
        assert_eq!(
            plan_before_fade(SIX_SECONDS, clock(start - 0.000_001, 200.0), None),
            FaderPlan::Recheck(SHORTEST_RECHECK)
        );
        assert_eq!(
            plan_before_fade(SIX_SECONDS, clock(start, 200.0), None),
            FaderPlan::Begin(window(start, end))
        );
        assert_eq!(
            plan_before_fade(SIX_SECONDS, clock(198.0, 200.0), None),
            FaderPlan::Begin(window(198.0, end))
        );
    }

    #[test]
    fn short_incoming_track_shortens_the_fade() {
        let end = audible_end(200.0);
        let capped_start = end - 2.0;
        let shortly_before = capped_start - 0.25;
        assert_eq!(
            plan_before_fade(SIX_SECONDS, clock(shortly_before, 200.0), Some(5.0)),
            FaderPlan::Recheck(Duration::from_secs_f64(capped_start - shortly_before))
        );
        assert_eq!(
            plan_before_fade(SIX_SECONDS, clock(capped_start, 200.0), Some(5.0)),
            FaderPlan::Begin(window(capped_start, end))
        );
        for unusable in [Some(0.0), Some(f64::NAN), Some(f64::INFINITY), None] {
            assert_eq!(
                plan_before_fade(SIX_SECONDS, clock(end - 6.0, 200.0), unusable),
                FaderPlan::Begin(window(end - 6.0, end))
            );
        }
    }

    #[test]
    fn never_begins_while_paused_or_without_timing() {
        let paused = DeckClock {
            is_paused: true,
            ..clock(199.0, 200.0)
        };
        let unknown_position = DeckClock {
            position_seconds: None,
            ..clock(0.0, 200.0)
        };
        let unknown_duration = DeckClock {
            duration_seconds: None,
            ..clock(199.0, 0.0)
        };
        for lead in [paused, unknown_position, unknown_duration] {
            assert_eq!(
                plan_before_fade(SIX_SECONDS, lead, None),
                FaderPlan::Recheck(LONGEST_RECHECK)
            );
        }
    }

    #[test]
    fn ramps_and_then_holds_until_the_outgoing_track_is_overdue() {
        let fade = window(194.0, 200.0);
        assert_eq!(
            plan_during_fade(fade, Some(3.0)),
            FaderPlan::Ramp(equal_power_gains(0.5))
        );
        assert_eq!(
            plan_during_fade(fade, Some(7.0)),
            FaderPlan::Ramp(equal_power_gains(1.0))
        );
        assert_eq!(plan_during_fade(fade, Some(7.5)), FaderPlan::Finish);
        assert_eq!(plan_during_fade(fade, None), FaderPlan::Finish);
    }
}
