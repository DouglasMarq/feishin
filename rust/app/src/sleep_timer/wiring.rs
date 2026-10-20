use slint::ComponentHandle;

use super::SleepMode;
use crate::controller::Command;
use crate::ui::{AppWindow, Context, PlayerState};

fn sleep_mode_for_minutes(minutes: i32) -> Option<SleepMode> {
    u32::try_from(minutes)
        .ok()
        .filter(|minutes| *minutes > 0)
        .map(SleepMode::Minutes)
}

pub fn install(window: &AppWindow, context: &Context) {
    let state = window.global::<PlayerState>();
    let start_context = context.clone();
    state.on_start_sleep_timer(move |minutes| {
        if let Some(mode) = sleep_mode_for_minutes(minutes) {
            start_context.send(Command::SleepTimer(Some(mode)));
        }
    });
    let end_of_track_context = context.clone();
    state.on_sleep_at_end_of_track(move || {
        end_of_track_context.send(Command::SleepTimer(Some(SleepMode::EndOfTrack)));
    });
    let cancel_context = context.clone();
    state.on_cancel_sleep_timer(move || cancel_context.send(Command::SleepTimer(None)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_a_positive_minute_count_the_timer_counts_down_those_minutes() {
        assert_eq!(sleep_mode_for_minutes(15), Some(SleepMode::Minutes(15)));
    }

    #[test]
    fn given_zero_or_negative_minutes_no_timer_starts() {
        assert_eq!(sleep_mode_for_minutes(0), None);
        assert_eq!(sleep_mode_for_minutes(-5), None);
    }
}
