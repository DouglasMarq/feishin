use std::time::Duration;

use super::gain::{FadeGains, equal_power_gains};
use crate::deck::DeckClock;

pub(crate) const FADE_STEP: Duration = Duration::from_millis(40);
const SHORTEST_RECHECK: Duration = Duration::from_millis(5);
const LONGEST_RECHECK: Duration = Duration::from_millis(500);
const OUTGOING_END_GRACE_SECONDS: f64 = 1.5;
pub(crate) const OUTPUT_BUFFER_SECONDS: f64 = 0.2;
const DRAIN_MARGIN_SECONDS: f64 = OUTPUT_BUFFER_SECONDS + 0.05;
const LONGEST_FADE_SHARE_OF_TRACK: f64 = 0.4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FadeWindow {
    start: f64,
    end: f64,
}

impl FadeWindow {
    pub(crate) fn planned(duration: Option<f64>, crossfade: Duration) -> Option<Self> {
        let duration = duration.filter(|seconds| seconds.is_finite())?;
        let audible_end = duration - DRAIN_MARGIN_SECONDS;
        let length = crossfade
            .as_secs_f64()
            .min(duration * LONGEST_FADE_SHARE_OF_TRACK)
            .min(audible_end);
        (length > 0.0).then_some(Self {
            start: audible_end - length,
            end: audible_end,
        })
    }

    pub(crate) fn has_begun(self, position: f64) -> bool {
        position >= self.start
    }

    pub(crate) fn starting_at(self, position: f64) -> Self {
        Self {
            start: position.clamp(self.start, self.end),
            end: self.end,
        }
    }

    pub(crate) fn progress(self, incoming_position: f64) -> f64 {
        match self.length() > 0.0 {
            true => (incoming_position / self.length()).clamp(0.0, 1.0),
            false => 1.0,
        }
    }

    fn is_overdue(self, incoming_position: f64) -> bool {
        incoming_position >= self.length() + OUTGOING_END_GRACE_SECONDS
    }

    fn length(self) -> f64 {
        self.end - self.start
    }

    fn time_until_start(self, position: f64) -> Duration {
        Duration::try_from_secs_f64(self.start - position).unwrap_or(Duration::ZERO)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum FaderPlan {
    Recheck(Duration),
    Begin(FadeWindow),
    Ramp(FadeGains),
    Finish,
}

pub(crate) fn plan_before_fade(crossfade: Duration, lead: DeckClock) -> FaderPlan {
    let window = FadeWindow::planned(lead.duration, crossfade);
    match (lead.position, window) {
        (Some(position), Some(window)) if !lead.paused => match window.has_begun(position) {
            true => FaderPlan::Begin(window.starting_at(position)),
            false => FaderPlan::Recheck(
                window
                    .time_until_start(position)
                    .clamp(SHORTEST_RECHECK, LONGEST_RECHECK),
            ),
        },
        _ => FaderPlan::Recheck(LONGEST_RECHECK),
    }
}

pub(crate) fn plan_during_fade(window: FadeWindow, incoming_position: Option<f64>) -> FaderPlan {
    match incoming_position {
        Some(position) if !window.is_overdue(position) => {
            FaderPlan::Ramp(equal_power_gains(window.progress(position)))
        }
        _ => FaderPlan::Finish,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIX_SECONDS: Duration = Duration::from_secs(6);

    fn clock(position: f64, duration: f64) -> DeckClock {
        DeckClock {
            position: Some(position),
            duration: Some(duration),
            paused: false,
        }
    }

    fn window(start: f64, end: f64) -> FadeWindow {
        FadeWindow { start, end }
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
        assert!(planned.is_some_and(|fade| fade.start > 5.0));
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
            plan_before_fade(SIX_SECONDS, clock(100.0, 200.0)),
            FaderPlan::Recheck(LONGEST_RECHECK)
        );
        assert_eq!(
            plan_before_fade(SIX_SECONDS, clock(start - 0.1, 200.0)),
            FaderPlan::Recheck(Duration::from_secs_f64(start - (start - 0.1)))
        );
        assert_eq!(
            plan_before_fade(SIX_SECONDS, clock(start - 0.000_001, 200.0)),
            FaderPlan::Recheck(SHORTEST_RECHECK)
        );
        assert_eq!(
            plan_before_fade(SIX_SECONDS, clock(start, 200.0)),
            FaderPlan::Begin(window(start, end))
        );
        assert_eq!(
            plan_before_fade(SIX_SECONDS, clock(198.0, 200.0)),
            FaderPlan::Begin(window(198.0, end))
        );
    }

    #[test]
    fn never_begins_while_paused_or_without_timing() {
        let paused = DeckClock {
            paused: true,
            ..clock(199.0, 200.0)
        };
        let unknown_position = DeckClock {
            position: None,
            ..clock(0.0, 200.0)
        };
        let unknown_duration = DeckClock {
            duration: None,
            ..clock(199.0, 0.0)
        };
        for lead in [paused, unknown_position, unknown_duration] {
            assert_eq!(
                plan_before_fade(SIX_SECONDS, lead),
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
